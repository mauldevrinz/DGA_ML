#!/usr/bin/env python3
import asyncio
from collections import deque
import websockets
import usb.core
import usb.util
import threading
import queue
import time
import os
import subprocess
import sys
import select
import base64
import math
import struct
import json
import re

from fuzzy_pid import FuzzyPIDController

# --- Rust SNN pipeline with FPGA hardware request ---
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SNN_MODELS_DIR = os.path.join(SCRIPT_DIR, "fpga", "snn_infer", "models")
SNN_FEATURE_SCRIPT = os.path.join(SCRIPT_DIR, "fpga", "snn_new", "extract_features.py")
SNN_INFER_BIN = os.environ.get(
    "SNN_INFER_BIN",
    os.path.join(SCRIPT_DIR, "fpga", "snn_infer", "target", "release", "snn_infer"),
)
_gas_classes = ["C2H2", "C2H4", "CH4", "H2", "OTHER"]
_snn_runner_lock = threading.Lock()
_feature_worker_lock = threading.Lock()
_feature_worker_proc = None

_sensor_history = deque(maxlen=288)
_sensor_history_lock = threading.Lock()
_cycle_recordings = {}
_cycle_recordings_lock = threading.Lock()
_cycle_sample_events = {}
_cycle_inference_launched = set()
_cycle_inference_enabled = False
_cycle_result_clients = set()
_cycle_inference_tasks = set()
_cycle_last_sequence = {}
_cycle_overflowed = set()
MAX_CYCLE_SAMPLES = 3600
_cycle_inference_generation = 0
_active_system_cycle = None
_cycle_min_number = 1
_stream_failure_reported = False
ADC_COUNT_TO_MV = (4.096 / 32768.0) * 1000.0
_kria_power_lock = threading.Lock()
_kria_power_samples = deque(maxlen=3600)
_kria_power_hwmon = None
_kria_power_status = "Power sensor has not been detected"

def update_sensor_history(sensor_row):
    """Keep rolling buffer of 16 MOS sensor readings (mV)."""
    with _sensor_history_lock:
        _sensor_history.append(list(sensor_row[:16]))


def parse_sensor_sample_frame(line):
    """Return (cycle, sequence, 16 ADC counts) from current or legacy Teensy frames."""
    fields = line.strip().split(",")
    if not fields or fields[0] != "SAMPLE":
        return None
    has_cycle_number = len(fields) == 35
    if len(fields) > 35:
        try:
            cycle_candidate = float(fields[1])
            sequence_candidate = float(fields[2])
            has_cycle_number = 0 <= cycle_candidate <= 100 and sequence_candidate > cycle_candidate
        except (IndexError, ValueError):
            has_cycle_number = False
    cycle_index, sequence_index, sensor_start = (1, 2, 3) if has_cycle_number else (None, 1, 2)
    try:
        cycle = int(float(fields[cycle_index])) if cycle_index is not None else 0
        sequence = int(float(fields[sequence_index]))
        sensor_counts = [float(value) for value in fields[sensor_start:sensor_start + 16]]
    except (IndexError, ValueError, OverflowError):
        return None
    if len(sensor_counts) != 16 or not all(math.isfinite(value) for value in sensor_counts):
        return None
    return cycle, sequence, sensor_counts

def _diagnose_iec60599(gas_confidences, predicted_fault=None):
    """
    Combines ML fault classification (T1, D1, N) with IEC 60599 diagnosis.
    Gas keys: C2H2, C2H4, CH4, H2, OTHER (confidence 0-100).
    """
    c2h2 = gas_confidences.get('C2H2', 0)
    c2h4 = gas_confidences.get('C2H4', 0)
    h2   = gas_confidences.get('H2', 0)
    ch4  = gas_confidences.get('CH4', 0)
    other = gas_confidences.get('OTHER', 0)

    # Prioritaskan prediksi Random Forest ML model jika valid
    if predicted_fault in ("N", "Normal"):
        return {"fault_code": "Normal", "fault_type": "No Fault (Normal Oil)",
                "description": "Transformer oil condition is normal", "severity": "normal"}
    elif predicted_fault == "D1":
        return {"fault_code": "D1", "fault_type": "Electrical Fault (Low Energy)",
                "description": "Low Energy Discharge — Sparking", "severity": "warning"}
    elif predicted_fault == "T1":
        return {"fault_code": "T1", "fault_type": "Thermal Fault (<300°C)",
                "description": "Low-temperature thermal fault (<300°C)", "severity": "caution"}

    # Rule-based fallback hanya jika ML tidak menghasilkan prediksi
    fault_total = c2h2 + c2h4 + h2 + ch4
    if fault_total < 15:
        return {"fault_code": "Normal", "fault_type": "No Fault",
                "description": "No fault detected / low dissolved gas concentration", "severity": "normal"}

    r1 = c2h2 / max(c2h4, 1.0)
    if c2h2 > 30 and h2 > 30:
        return {"fault_code": "D1", "fault_type": "Electrical Fault",
                "description": f"High Energy Discharge — Sparking/Arcing (C2H2/C2H4={r1:.2f})", "severity": "critical"}
    if c2h4 > 40:
        return {"fault_code": "T1", "fault_type": "Thermal Fault",
                "description": "Thermal Fault detected (<300°C)", "severity": "warning"}

    return {"fault_code": "Normal", "fault_type": "No Fault",
            "description": "Dissolved gas levels within acceptable limits", "severity": "normal"}


def _read_worker_line(proc, timeout):
    ready, _, _ = select.select([proc.stdout], [], [], timeout)
    return proc.stdout.readline() if ready else None


def _start_feature_worker_locked(timeout):
    global _feature_worker_proc
    if _feature_worker_proc is not None and _feature_worker_proc.poll() is None:
        return _feature_worker_proc
    _feature_worker_proc = subprocess.Popen(
        [sys.executable, "-u", SNN_FEATURE_SCRIPT], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1,
    )
    ready = _read_worker_line(_feature_worker_proc, timeout)
    if not ready:
        _feature_worker_proc.terminate()
        _feature_worker_proc = None
        raise TimeoutError("Python tsfresh worker did not become ready")
    try:
        if not json.loads(ready).get("ready"):
            raise RuntimeError(f"Unexpected Python tsfresh worker handshake: {ready[:200]}")
    except Exception:
        _feature_worker_proc.terminate()
        _feature_worker_proc = None
        raise
    return _feature_worker_proc


def warm_snn_feature_worker(timeout=60.0):
    """Load numpy/pandas/tsfresh once during backend startup."""
    with _feature_worker_lock:
        _start_feature_worker_locked(timeout)


def _extract_python_features(recording, timeout):
    global _feature_worker_proc
    with _feature_worker_lock:
        proc = _start_feature_worker_locked(timeout)
        try:
            proc.stdin.write(json.dumps({"recording": recording}, allow_nan=False) + "\n")
            proc.stdin.flush()
            line = _read_worker_line(proc, timeout)
            if not line:
                raise RuntimeError("Python tsfresh worker exited or timed out")
            return json.loads(line)
        except Exception:
            if proc.poll() is None:
                proc.terminate()
            _feature_worker_proc = None
            raise


def _attach_power_summary(result, started, ended):
    for key in ("kria_power_w", "kria_peak_power_w", "kria_vbus_v", "kria_energy_mj"):
        result.pop(key, None)
    with _kria_power_lock:
        samples = [sample for sample in _kria_power_samples if started <= sample[0] <= ended]
        status = _kria_power_status
    result["power_sample_count"] = len(samples)
    if not samples:
        result["power_status"] = status if status != "ok" else "No power readings captured during inference"
        return
    average_power = sum(sample[3] for sample in samples) / len(samples)
    result["kria_power_w"] = round(average_power, 4)
    result["kria_peak_power_w"] = round(max(sample[3] for sample in samples), 4)
    result["kria_vbus_v"] = round(sum(sample[1] for sample in samples) / len(samples), 4)
    result["kria_energy_mj"] = round(average_power * (ended - started) * 1000.0, 4)
    result["power_status"] = "ok"


def snn_rust_inference(recording, use_hw=True, timeout=60.0):
    """Extract the model's Python tsfresh features, then run SNN/RF via Rust."""
    environment = os.environ.copy()
    environment["SNN_MODELS_DIR"] = SNN_MODELS_DIR
    started = time.monotonic()
    try:
        feature_result = _extract_python_features(recording, timeout)
        if not feature_result.get("ok"):
            return {"ok": False, "error": feature_result.get("error", "Python tsfresh feature extraction failed")}
        request = json.dumps({
            "features": feature_result["features"],
            "feature_ms": feature_result["feature_ms"],
            "use_hw": use_hw,
        }, allow_nan=False)
        with _snn_runner_lock:
            completed = subprocess.run(
                [SNN_INFER_BIN], input=request + "\n", text=True,
                capture_output=True, timeout=timeout, env=environment, check=False,
            )
        if completed.returncode != 0:
            return {"ok": False, "error": f"SNN runner exited with code {completed.returncode}: {completed.stderr[-1000:]}"}
        output_lines = [line for line in completed.stdout.splitlines() if line.strip()]
        if not output_lines:
            return {"ok": False, "error": f"SNN runner returned no result: {completed.stderr[-1000:]}"}
        result = json.loads(output_lines[-1])
        if not result.get("ok"):
            return result

        confidences = result.get("gas_confidences", {})
        gas_names = result.get("gas_names", _gas_classes)
        result["outputs"] = result.get("outputs") or [
            float(confidences.get(name, 0.0)) / 100.0 for name in gas_names
        ]
        result["feature_latency_ms"] = result.get("feature_ms")
        ended = time.monotonic()
        result["total_latency_ms"] = round((ended - started) * 1000.0, 2)
        _attach_power_summary(result, started, ended)
        result["iec_diagnosis"] = _diagnose_iec60599(
            confidences, result.get("predicted_fault")
        )
        result["hardware_requested"] = bool(use_hw)
        return result
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": f"Inference pipeline timed out after {timeout:.1f}s"}
    except Exception as exc:
        return {"ok": False, "error": f"Could not run SNN FPGA pipeline: {exc}"}


def snn_fpga_inference(sensor_values):
    """Wrapper untuk panggilan single frame live dari GUI."""
    if isinstance(sensor_values, list) and sensor_values and isinstance(sensor_values[0], list):
        rec = sensor_values
    else:
        # Gunakan rolling history jika tersedia, tambahkan frame terbaru
        update_sensor_history(sensor_values)
        with _sensor_history_lock:
            rec = list(_sensor_history)
        if len(rec) < 10:
            rec = [sensor_values for _ in range(10)]
    return snn_rust_inference(rec, use_hw=True)

setpoint = 0.0
fuzzy_controller = FuzzyPIDController(kp=5.0, ki=0.5, kd=1.0)

dev = None
usb_lock = threading.Lock()
shutdown_event = threading.Event()

RUST_BIN = os.path.join(os.path.dirname(os.path.abspath(__file__)), "persistence_rs", "target", "release", "persistence_rs")
persister_proc = None
try:
    persister_proc = subprocess.Popen([RUST_BIN], stdin=subprocess.PIPE, text=True)
except Exception as e:
    print(f"Warning: Could not start Rust persister: {e}")
USB_INTERFACE = 1
USB_IN_ENDPOINT = 0x82
USB_OUT_ENDPOINT = 0x01
USB_WRITE_TIMEOUT_MS = 200
command_condition = threading.Condition()
urgent_commands = deque()
pending_pwr = None
pending_heartbeat = False
phase_active = False
phase_owner = None
phase_task = None
phase_ack_future = None


def _usb_write(message):
    """Write one command from the dedicated USB writer thread."""
    if dev is None:
        return False
    payload = message if message.endswith('\n') else f"{message}\n"
    with usb_lock:
        dev.write(
            USB_OUT_ENDPOINT,
            payload.encode('utf-8'),
            timeout=USB_WRITE_TIMEOUT_MS,
        )
    return True


def queue_command(message, automatic=False):
    """Queue commands, coalescing heartbeats and automatic PWR updates."""
    global pending_pwr, pending_heartbeat, phase_active
    with command_condition:
        if message == "HEARTBEAT":
            pending_heartbeat = True
        elif message.startswith("PHASE="):
            phase_active = message.split("=", 1)[1].strip() != "OFF"
            urgent_commands.append(message)
        elif automatic:
            pending_pwr = message
        else:
            urgent_commands.append(message)
        command_condition.notify()


def usb_write_loop():
    global pending_pwr, pending_heartbeat
    MAX_RETRIES = 5
    while not shutdown_event.is_set():
        with command_condition:
            command_condition.wait_for(
                lambda: shutdown_event.is_set()
                or urgent_commands
                or pending_heartbeat
                or pending_pwr is not None
            )
            if shutdown_event.is_set():
                break
            if urgent_commands:
                item = urgent_commands.popleft()
                # Items are (message, retries_left) tuples or plain strings
                if isinstance(item, tuple):
                    message, retries_left = item
                else:
                    message, retries_left = item, MAX_RETRIES
                automatic = False
            elif pending_heartbeat:
                message = "HEARTBEAT"
                pending_heartbeat = False
                retries_left = MAX_RETRIES
                automatic = False
            else:
                message = pending_pwr
                pending_pwr = None
                retries_left = 0
                automatic = True

        try:
            _usb_write(message)
        except usb.core.USBError as e:
            if e.errno == 110 or 'timeout' in str(e).lower():
                if automatic:
                    print("PWR write timed out; dropping stale PID update")
                elif retries_left > 0:
                    print(f"USB write timed out, retrying ({retries_left} left): {message}")
                    with command_condition:
                        urgent_commands.appendleft((message, retries_left - 1))
                        command_condition.notify()
                else:
                    print(f"USB write timed out, giving up: {message}")
            else:
                print(f"USB command write error for {message}: {e}")
        except Exception as e:
            print(f"USB command write error for {message}: {e}")


async def device_heartbeat_loop():
    """Keep an active Teensy run alive independently of browser timer throttling."""
    while not shutdown_event.is_set():
        with command_condition:
            active = phase_active
        if active and dev is not None:
            queue_command("HEARTBEAT")
        await asyncio.sleep(1.0)


def _resolve_phase_ack(phase):
    future = phase_ack_future
    if future is not None and not future.done():
        future.set_result(phase)


async def run_system_plan(websocket, phases, pump_pwm):
    """Run all requested cycles on Kria so browser timer throttling cannot stall phases."""
    global phase_task, phase_owner, phase_ack_future, _active_system_cycle
    phase_owner = websocket
    previous_cycle = None
    try:
        queue_command("SYSTEM_ARM")
        queue_command(f"PWM={pump_pwm}")
        for phase_index, item in enumerate(phases):
            if websocket not in clients or phase_owner is not websocket:
                break
            cycle_number = item["cycle"]
            _active_system_cycle = cycle_number
            phase = item["phase"]
            duration = item["duration_sec"]
            if cycle_number != previous_cycle:
                queue_command(f"CYCLE={cycle_number}")
                previous_cycle = cycle_number
                if _cycle_inference_enabled and cycle_number >= _cycle_min_number:
                    with _cycle_recordings_lock:
                        _cycle_sample_events.setdefault(cycle_number, threading.Event()).clear()
            if phase == "INJECT":
                queue_command(f"PWM={pump_pwm}")

            loop = asyncio.get_running_loop()
            phase_ack_future = loop.create_future()
            queue_command(f"PHASE={phase}")
            try:
                acknowledged = await asyncio.wait_for(phase_ack_future, timeout=5.0)
            except asyncio.TimeoutError:
                await _abort_system_after_stream_failure(
                    f"Teensy tidak mengakui fase {phase} dalam 5 detik. Periksa USB dan status Teensy."
                )
                return
            finally:
                phase_ack_future = None
            if acknowledged != phase:
                raise RuntimeError(f"Unexpected phase acknowledgement: {acknowledged}")

            await broadcast_message(json.dumps({
                "type": "SYSTEM_PHASE",
                "phase": phase,
                "cycle": cycle_number,
                "duration_sec": duration,
            }))
            await asyncio.sleep(duration)

            # Run inference once, after the final planned phase in a cycle.
            next_item = phases[phase_index + 1] if phase_index + 1 < len(phases) else None
            if next_item is None or next_item["cycle"] != cycle_number:
                if _cycle_inference_enabled and cycle_number >= _cycle_min_number:
                    with _cycle_recordings_lock:
                        event = _cycle_sample_events.setdefault(cycle_number, threading.Event())
                        last_sequence = _cycle_last_sequence.get(cycle_number)
                        overflowed = cycle_number in _cycle_overflowed
                        event.clear()
                    if not overflowed:
                        got_final_frame = await asyncio.to_thread(event.wait, 3.0)
                        with _cycle_recordings_lock:
                            latest_sequence = _cycle_last_sequence.get(cycle_number)
                        if not got_final_frame or latest_sequence is None or latest_sequence == last_sequence:
                            message = (
                                f"Cycle {cycle_number} dibatalkan: frame sensor penutup tidak diterima "
                                "dalam 3 detik setelah fase terakhir. Periksa koneksi USB/Teensy dan stream sensor."
                            )
                            await _abort_system_after_stream_failure(message)
                            return
                        # Allow the USB reader to drain any bytes queued in the same transfer.
                        await asyncio.sleep(0.05)
                launch_cycle_inference(cycle_number)
                with _cycle_recordings_lock:
                    for old_cycle in [key for key in _cycle_recordings if key <= cycle_number]:
                        _cycle_recordings.pop(old_cycle, None)
                    _cycle_overflowed.discard(cycle_number)
                    _cycle_last_sequence.pop(cycle_number, None)
                    _cycle_sample_events.pop(cycle_number, None)
                _active_system_cycle = None

        if phase_owner is websocket:
            await broadcast_message(json.dumps({"type": "SYSTEM_COMPLETE"}))
    except Exception as exc:
        await broadcast_message(json.dumps({"type": "SYSTEM_ERROR", "message": str(exc)}))
    except asyncio.CancelledError:
        raise
    finally:
        phase_ack_future = None
        if phase_owner is websocket:
            queue_command("PHASE=OFF")
            phase_owner = None
        _active_system_cycle = None
        if phase_task is asyncio.current_task():
            phase_task = None


def launch_cycle_inference(cycle_number):
    if not _cycle_inference_enabled:
        return
    generation = _cycle_inference_generation
    with _cycle_recordings_lock:
        if cycle_number in _cycle_inference_launched:
            print(f"Skipping duplicate cycle inference launch for cycle {cycle_number}")
            return
        _cycle_inference_launched.add(cycle_number)
        recording = _cycle_recordings.pop(cycle_number, [])
        overflowed = cycle_number in _cycle_overflowed
        _cycle_overflowed.discard(cycle_number)
        _cycle_last_sequence.pop(cycle_number, None)
    task = asyncio.create_task(infer_completed_cycle(cycle_number, recording, generation, overflowed))
    _cycle_inference_tasks.add(task)
    task.add_done_callback(_cycle_inference_tasks.discard)


async def infer_completed_cycle(cycle_number, recording, generation, overflowed=False):
    try:
        if overflowed:
            result = {
                "type": "SNN_RESULT", "ok": False,
                "error": f"Cycle {cycle_number} melewati batas {MAX_CYCLE_SAMPLES} sampel; inference cycle dibatalkan.",
                "cycle": cycle_number, "inference_mode": "cycle",
            }
        elif len(recording) < 10:
            result = {
                "type": "SNN_RESULT", "ok": False,
                "error": f"Cycle {cycle_number} hanya memiliki {len(recording)} sampel; minimal 10 diperlukan.",
                "cycle": cycle_number, "inference_mode": "cycle",
            }
        else:
            loop = asyncio.get_running_loop()
            async with inference_gate:
                result = await loop.run_in_executor(None, snn_fpga_inference_safe, recording)
            result.update({"type": "SNN_RESULT", "cycle": cycle_number, "inference_mode": "cycle"})
    except Exception as exc:
        result = {
            "type": "SNN_RESULT", "ok": False,
            "error": f"Cycle {cycle_number} inference gagal: {exc}",
            "cycle": cycle_number, "inference_mode": "cycle",
        }
    result["cycle_sample_count"] = len(recording)
    if generation == _cycle_inference_generation and _cycle_inference_enabled:
        await broadcast_cycle_result(result)


async def broadcast_cycle_result(result):
    payload = json.dumps(result)
    stale = []
    for client in tuple(_cycle_result_clients):
        try:
            await client.send(payload)
        except Exception:
            stale.append(client)
    for client in stale:
        _cycle_result_clients.discard(client)


def _notify_serial_stream_failure(message):
    """Schedule a single system-plan abort from the USB reader thread."""
    global _stream_failure_reported
    if phase_task is None or phase_task.done() or _stream_failure_reported:
        return
    _stream_failure_reported = True
    asyncio.create_task(_abort_system_after_stream_failure(message))


async def _abort_system_after_stream_failure(message):
    global phase_task, phase_owner, phase_active, _active_system_cycle
    cycle_number = _active_system_cycle
    task = phase_task
    if task is not None and task is not asyncio.current_task() and not task.done():
        task.cancel()
    queue_command("PHASE=OFF")
    with command_condition:
        phase_active = False
        command_condition.notify_all()
    await broadcast_message(json.dumps({"type": "SYSTEM_ERROR", "message": message}))
    if cycle_number is not None and _cycle_inference_enabled:
        await broadcast_cycle_result({
            "type": "SNN_RESULT", "ok": False, "error": message,
            "cycle": cycle_number, "inference_mode": "cycle",
        })
    with _cycle_recordings_lock:
        if cycle_number is not None:
            _cycle_recordings.pop(cycle_number, None)
            _cycle_last_sequence.pop(cycle_number, None)
            _cycle_overflowed.discard(cycle_number)
            _cycle_sample_events.pop(cycle_number, None)
    if task is None or task.done():
        phase_task = None
        phase_owner = None
        _active_system_cycle = None


def connect_serial():
    global dev
    if shutdown_event.is_set():
        return
    try:
        new_dev = usb.core.find(idVendor=0x16c0, idProduct=0x0483)
        if new_dev is not None:
            if new_dev.is_kernel_driver_active(1):
                new_dev.detach_kernel_driver(1)
            new_dev.set_configuration()
            usb.util.claim_interface(new_dev, USB_INTERFACE)
            # Set DTR/RTS to signal terminal is connected
            new_dev.ctrl_transfer(0x21, 0x22, 0x03, 0, None)
            dev = new_dev
            print("Connected to Teensy via PyUSB")
        else:
            dev = None
    except Exception as e:
        if not shutdown_event.is_set():
            print(f"Failed to connect to USB: {e}")
        dev = None


def close_serial():
    global dev
    with usb_lock:
        if dev is None:
            return
        try:
            usb.util.release_interface(dev, USB_INTERFACE)
        except usb.core.USBError:
            pass
        finally:
            usb.util.dispose_resources(dev)
            dev = None

connect_serial()
clients = set()
inference_gate = asyncio.Lock()

SAVE_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "Saved Graphs")
os.makedirs(SAVE_DIR, exist_ok=True)

GAS_COLORS = [
    '#C81C1C', '#E27636', '#C89D1C', '#CCE236',
    '#72C81C', '#4BE236', '#1CC847', '#36E2A1',
    '#1CC8C8', '#36A1E2', '#1C47C8', '#4B36E2',
    '#721CC8', '#CC36E2', '#C81C9D', '#E23676',
]

def generate_gnuplot_graph(data, baselines, session_name, sensor_names):
    """Generate a normalized Rs/R0 graph using matplotlib (gnuplot style), save as PNG."""
    import re
    import matplotlib
    matplotlib.use('Agg')  # Non-interactive backend, safe for server use
    import matplotlib.pyplot as plt
    import matplotlib.ticker as ticker

    if not data or not baselines:
        return {'ok': False, 'error': 'No data or baselines provided.'}

    safe_name = re.sub(r'[^a-zA-Z0-9_\-]', '_', session_name or 'session')
    timestamp = time.strftime('%Y%m%d_%H%M%S')
    output_png = os.path.join(SAVE_DIR, f"{safe_name}_{timestamp}.png")

    # Compute normalized data
    num_sensors = len(sensor_names)
    times = [pt.get('time', i) for i, pt in enumerate(data)]
    norm_data = []
    for i in range(num_sensors):
        col = []
        for pt in data:
            raw  = pt.get(f'mos{i}', 0) or 0
            base = baselines.get(f'mos{i}', 1) or 1
            col.append(raw / base)
        norm_data.append(col)

    # --- Style to mimic gnuplot classic ---
    with plt.style.context('classic'):
        fig, ax = plt.subplots(figsize=(16, 7), dpi=120)
        fig.patch.set_facecolor('white')
        ax.set_facecolor('white')

        for i, name in enumerate(sensor_names):
            color = GAS_COLORS[i % len(GAS_COLORS)]
            ax.plot(times, norm_data[i], label=name, color=color, linewidth=1.8)

        ax.set_title(
            f'Normalized Gas Sensor Response (Rs/R0)\nSession: {session_name}',
            fontsize=13, fontweight='bold', pad=12
        )
        ax.set_xlabel('Time (s)', fontsize=11)
        ax.set_ylabel('Rs / R0', fontsize=11)
        ax.axhline(y=1.0, color='gray', linestyle='--', linewidth=1, alpha=0.7, label='Baseline (1.0)')
        ax.grid(True, linestyle='--', linewidth=0.5, alpha=0.6, color='gray')
        ax.xaxis.set_minor_locator(ticker.AutoMinorLocator())
        ax.yaxis.set_minor_locator(ticker.AutoMinorLocator())
        ax.tick_params(which='both', direction='in', top=True, right=True)
        plotted_values = [
            value
            for sensor_values in norm_data
            for value in sensor_values
            if math.isfinite(value)
        ]
        if not plotted_values:
            plt.close(fig)
            return {'ok': False, 'error': 'No finite normalized values to plot.'}

        y_min = min(plotted_values)
        y_max = max(plotted_values)
        y_range = y_max - y_min
        margin = max(y_range * 0.08, 0.01)
        if y_range == 0:
            margin = max(abs(y_min) * 0.08, 0.01)
        ax.set_ylim(y_min - margin, y_max + margin)

        # Legend outside right
        ax.legend(
            loc='upper left', bbox_to_anchor=(1.01, 1.0),
            fontsize=8, frameon=True, framealpha=0.9,
            borderpad=0.8, labelspacing=0.4
        )

        fig.tight_layout(rect=[0, 0, 0.82, 1])
        fig.savefig(output_png, dpi=120, bbox_inches='tight')
        plt.close(fig)

    with open(output_png, 'rb') as image_file:
        image_b64 = base64.b64encode(image_file.read()).decode('ascii')

    return {
        'ok': True,
        'filename': os.path.basename(output_png),
        'b64_image': image_b64,
    }


def snn_fpga_inference_safe(sensor_values, timeout=8.0):
    recording = sensor_values
    if not (isinstance(recording, list) and recording and isinstance(recording[0], list)):
        update_sensor_history(sensor_values)
        with _sensor_history_lock:
            recording = list(_sensor_history)
    if len(recording) < 10:
        return {"ok": False, "error": f"Inference requires at least 10 sensor frames; received {len(recording)}"}
    return snn_rust_inference(recording, use_hw=True, timeout=max(timeout, 60.0))

async def handler(websocket):
    global setpoint, fuzzy_controller, phase_active, phase_owner, phase_task
    clients.add(websocket)
    print(f"Client connected. Total clients: {len(clients)}")
    try:
        async for message in websocket:
            # Detect JSON commands from GUI (not serial commands)
            if message.startswith('{'):
                import json
                cmd = {}
                try:
                    cmd = json.loads(message)
                    if cmd.get('type') == 'CHAMBER_CTRL':
                        setpoint = float(cmd.get('setpoint', setpoint))
                        kp = float(cmd.get('kp', 5.0))
                        ki = float(cmd.get('ki', 0.5))
                        kd = float(cmd.get('kd', 1.0))
                        fuzzy_controller = FuzzyPIDController(kp=kp, ki=ki, kd=kd)
                        print(f"Updated Fuzzy PID: setpoint={setpoint}, Kp={kp}, Ki={ki}, Kd={kd}")
                        if dev is not None:
                            queue_command(f"SETPOINT={setpoint}")

                        await websocket.send(json.dumps({'ok': True, 'type': 'CHAMBER_CTRL_ACK'}))
                        continue

                    if cmd.get('type') == 'START_SYSTEM_PLAN':
                        global _stream_failure_reported
                        if phase_task is not None and not phase_task.done():
                            raise ValueError("A system run is already active. Stop it before starting another run.")
                        phases = cmd.get('phases')
                        pump_pwm = max(0, min(100, int(cmd.get('pump_pwm', 0))))
                        valid_phases = {"IDLE", "INJECT", "PURGE"}
                        if not isinstance(phases, list) or not phases or len(phases) > 600:
                            raise ValueError("The system plan must contain between 1 and 600 phases.")
                        plan = []
                        for item in phases:
                            if not isinstance(item, dict):
                                raise ValueError("Each system phase must be an object.")
                            phase = str(item.get("phase", ""))
                            cycle = int(item.get("cycle", 0))
                            duration_sec = int(item.get("duration_sec", 0))
                            if phase not in valid_phases or cycle < 1 or duration_sec < 1:
                                raise ValueError("Invalid phase, cycle number, or phase duration in the system plan.")
                            plan.append({"phase": phase, "cycle": cycle, "duration_sec": duration_sec})
                        with _cycle_recordings_lock:
                            _cycle_recordings.clear()
                            _cycle_last_sequence.clear()
                            _cycle_overflowed.clear()
                            _cycle_sample_events.clear()
                            _cycle_inference_launched.clear()
                        _stream_failure_reported = False
                        phase_task = asyncio.create_task(run_system_plan(websocket, plan, pump_pwm))
                        await websocket.send(json.dumps({"type": "SYSTEM_PLAN_ACCEPTED", "phases": len(plan)}))
                        continue

                    if cmd.get('type') == 'STOP_SYSTEM':
                        if phase_task is not None and not phase_task.done():
                            phase_task.cancel()
                        phase_task = None
                        phase_owner = None
                        with _cycle_recordings_lock:
                            _cycle_recordings.clear()
                            _cycle_last_sequence.clear()
                            _cycle_overflowed.clear()
                            _cycle_sample_events.clear()
                            _cycle_inference_launched.clear()
                        queue_command("PHASE=OFF")
                        await broadcast_message(json.dumps({"type": "SYSTEM_STOPPED"}))
                        continue

                    if cmd.get('type') == 'SET_CYCLE_INFERENCE':
                        global _cycle_inference_enabled, _cycle_inference_generation, _cycle_min_number
                        if cmd.get('enabled'):
                            _cycle_inference_enabled = True
                            _cycle_min_number = (_active_system_cycle + 1) if _active_system_cycle is not None else 1
                            _cycle_result_clients.add(websocket)
                        else:
                            _cycle_inference_enabled = False
                            _cycle_inference_generation += 1
                            _cycle_min_number = 1
                            _cycle_result_clients.clear()
                            with _cycle_recordings_lock:
                                _cycle_recordings.clear()
                                _cycle_last_sequence.clear()
                                _cycle_overflowed.clear()
                                _cycle_sample_events.clear()
                        await websocket.send(json.dumps({
                            "type": "CYCLE_INFERENCE_ACK", "enabled": _cycle_inference_enabled
                        }))
                        continue

                    if cmd.get('type') == 'GET_CYCLE_INFERENCE':
                        if _cycle_inference_enabled:
                            _cycle_result_clients.add(websocket)
                        await websocket.send(json.dumps({
                            "type": "CYCLE_INFERENCE_ACK", "enabled": _cycle_inference_enabled
                        }))
                        continue

                    if cmd.get('type') == 'SAVE_GRAPH':
                        result = generate_gnuplot_graph(
                            data=cmd['data'],
                            baselines=cmd['baselines'],
                            session_name=cmd['sessionName'],
                            sensor_names=cmd['sensorNames'],
                        )
                        await websocket.send(json.dumps(result))

                    if cmd.get('type') == 'SNN_CSV_INFER':
                        rows = cmd.get('rows', [])
                        await websocket.send(json.dumps({'type':'CSV_PROGRESS','stage':'validating','progress':10,'message':'Validating CSV data'}))
                        if len(rows) < 10: raise ValueError(f"CSV membutuhkan minimal 10 baris sensor, diterima {len(rows)}")
                        if len(rows) > MAX_CYCLE_SAMPLES:
                            raise ValueError(f"CSV maksimal {MAX_CYCLE_SAMPLES} baris, diterima {len(rows)}")
                        request_t0 = time.monotonic()
                        # Ambil nilai mos0..mos15 (mV) dari dictionary rows
                        recording = []
                        for r in rows:
                            row_mos = [float(r.get(f"mos{i}", r.get(f"mos{i}_mV", r.get(f"MOS{i}", 0.0)))) for i in range(16)]
                            recording.append(row_mos)

                        await websocket.send(json.dumps({'type':'CSV_PROGRESS','stage':'fpga','progress':50,'message':'Running SNN Inference'}))
                        loop = asyncio.get_running_loop()
                        result = await loop.run_in_executor(
                            None, snn_rust_inference, recording, True, 120.0
                        )
                        request_end = time.monotonic()
                        result['total_latency_ms'] = round((request_end - request_t0) * 1000, 2)
                        if result.get('ok'):
                            _attach_power_summary(result, request_t0, request_end)

                        result['type'] = 'SNN_RESULT'
                        result['inference_mode'] = 'csv'
                        await websocket.send(json.dumps(result))

                    if cmd.get('type') == 'SNN_INFER':
                        # Run SNN FPGA inference with sensor data
                        sensor_values = cmd.get('sensors', [])
                        loop = asyncio.get_running_loop()
                        if inference_gate.locked():
                            result = {"ok": False, "error": "FPGA inference busy; request skipped"}
                        else:
                            async with inference_gate:
                                result = await loop.run_in_executor(
                                    None, snn_fpga_inference_safe, sensor_values
                                )
                        result['type'] = 'SNN_RESULT'
                        await websocket.send(json.dumps(result))
                except Exception as e:
                    import traceback
                    error_type = 'SYSTEM_ERROR' if cmd.get('type') in {'START_SYSTEM_PLAN', 'STOP_SYSTEM'} else 'SNN_RESULT'
                    await websocket.send(json.dumps({'type': error_type, 'ok': False, 'message': str(e), 'error': str(e), 'trace': traceback.format_exc()}))
                continue

            # Otherwise: forward to Teensy via USB
            print(f"Command from GUI: {message}")
            if dev is not None:
                if message.startswith("PHASE="):
                    with command_condition:
                        phase_owner = websocket if message.split("=", 1)[1].strip() != "OFF" else None
                queue_command(message)
            else:
                print("USB not connected, ignoring command.")
    except websockets.exceptions.ConnectionClosed:
        pass
    finally:
        _cycle_result_clients.discard(websocket)
        clients.remove(websocket)
        with command_condition:
            if phase_active and phase_owner is websocket:
                phase_active = False
                phase_owner = None
                urgent_commands.appendleft("PHASE=OFF")
                command_condition.notify_all()
                if phase_task is not None and not phase_task.done():
                    phase_task.cancel()
        print("Client disconnected.")

def serial_read_loop(loop, outbound_queue):
    global dev, phase_active, phase_owner
    buffer = bytearray()

    # --- Dedicated persistence writer thread ---
    persist_queue = queue.Queue(maxsize=2048)

    def _persist_writer():
        """Drain persist_queue -> persister_proc.stdin in a separate thread
        so blocking flush() never stalls the USB read loop."""
        while not shutdown_event.is_set():
            try:
                line = persist_queue.get(timeout=1.0)
            except queue.Empty:
                continue
            if persister_proc and persister_proc.poll() is None:
                try:
                    persister_proc.stdin.write(line + "\n")
                    persister_proc.stdin.flush()
                except IOError:
                    pass

    persist_thread = threading.Thread(target=_persist_writer, daemon=True)
    persist_thread.start()

    while not shutdown_event.is_set():
        if dev is not None:
            try:
                # Read from Bulk IN endpoint (0x82) with a short timeout to prevent lock hogging
                with usb_lock:
                    try:
                        data = dev.read(USB_IN_ENDPOINT, 64, timeout=10)
                    except usb.core.USBError as e:
                        if e.errno == 110 or 'timeout' in str(e).lower():
                            data = None
                        else:
                            raise
                # Yield briefly so the USB writer thread can acquire the lock
                time.sleep(0.002)
                if data:
                    buffer.extend(data)
                    # Guard against unbounded buffer growth if newlines go missing
                    if len(buffer) > 4096:
                        buffer.clear()
                        continue
                    # Process lines if \n is present
                    while b'\n' in buffer:
                        line, buffer = buffer.split(b'\n', 1)
                        decoded = line.decode('utf-8', errors='ignore').strip()
                        if decoded:
                            phase_ack = re.fullmatch(r"ACK PHASE = (IDLE|INJECT|PURGE|OFF)", decoded)
                            if phase_ack:
                                loop.call_soon_threadsafe(_resolve_phase_ack, phase_ack.group(1))
                            if decoded == "SYSTEM_WATCHDOG=TRIPPED":
                                loop.call_soon_threadsafe(
                                    _notify_serial_stream_failure,
                                    "Teensy watchdog menghentikan system run. Periksa heartbeat dan koneksi USB sebelum mencoba lagi."
                                )
                            if decoded.startswith("SAMPLE,"):
                                parsed = parse_sensor_sample_frame(decoded)
                                if parsed:
                                    cycle_number, sequence, mos_counts = parsed
                                    mos_vals_mv = [value * ADC_COUNT_TO_MV for value in mos_counts]
                                    update_sensor_history(mos_vals_mv)
                                    if cycle_number > 0:
                                        with _cycle_recordings_lock:
                                            if _cycle_inference_enabled and cycle_number >= _cycle_min_number:
                                                sample_event = _cycle_sample_events.setdefault(cycle_number, threading.Event())
                                                if cycle_number not in _cycle_overflowed:
                                                    last_sequence = _cycle_last_sequence.get(cycle_number)
                                                    if last_sequence is None or sequence > last_sequence:
                                                        _cycle_last_sequence[cycle_number] = sequence
                                                        rows = _cycle_recordings.setdefault(cycle_number, [])
                                                        if len(rows) >= MAX_CYCLE_SAMPLES:
                                                            _cycle_recordings.pop(cycle_number, None)
                                                            _cycle_overflowed.add(cycle_number)
                                                            _cycle_last_sequence.pop(cycle_number, None)
                                                        else:
                                                            rows.append(mos_vals_mv)
                                                            sample_event.set()
                            # Enqueue for persistence (non-blocking)
                            try:
                                persist_queue.put_nowait(decoded)
                            except queue.Full:
                                pass  # drop if persistence can't keep up

                            loop.call_soon_threadsafe(enqueue_serial_message, outbound_queue, decoded)
            except usb.core.USBError as e:
                # Timeout is normal if no data, error code usually 110
                if e.errno == 110 or 'timeout' in str(e).lower():
                    continue
                elif shutdown_event.is_set():
                    break
                else:
                    print(f"USB read error: {e}")
                    loop.call_soon_threadsafe(
                        _notify_serial_stream_failure,
                        f"Koneksi USB sensor terputus saat system run: {e}"
                    )
                    dev = None
                    buffer.clear()  # Clear stale bytes to prevent frame contamination on reconnect
                    time.sleep(1)
            except Exception as e:
                if shutdown_event.is_set():
                    break
                print(f"USB disconnected or error: {e}")
                loop.call_soon_threadsafe(
                    _notify_serial_stream_failure,
                    f"Koneksi USB sensor terputus saat system run: {e}"
                )
                dev = None
                buffer.clear()  # Clear stale bytes to prevent frame contamination on reconnect
                time.sleep(1)
        else:
            shutdown_event.wait(3)
            connect_serial()

async def kria_power_loop():
    import glob
    global _kria_power_hwmon, _kria_power_status
    next_broadcast = 0.0
    next_scan = 0.0
    while not shutdown_event.is_set():
        now = time.monotonic()
        try:
            if _kria_power_hwmon is None or now >= next_scan or not os.path.isdir(_kria_power_hwmon):
                _kria_power_hwmon = None
                next_scan = now + 5.0
                for candidate in glob.glob("/sys/class/hwmon/hwmon*"):
                    try:
                        with open(os.path.join(candidate, "name"), "r") as f:
                            chip = f.read().strip().lower()
                        if chip.startswith("ina"):
                            _kria_power_hwmon = candidate
                            break
                    except OSError:
                        continue
            if _kria_power_hwmon is None:
                raise FileNotFoundError("No INA-family hwmon device found")

            v = _read_hwmon_float(_kria_power_hwmon, ("in1_input",)) / 1000.0
            i = _read_hwmon_float(_kria_power_hwmon, ("curr1_input",)) / 1000.0
            p = _read_hwmon_float(_kria_power_hwmon, ("power1_input",)) / 1e6
            sample_time = time.monotonic()
            with _kria_power_lock:
                _kria_power_samples.append((sample_time, v, i, p))
                _kria_power_status = "ok"
            if sample_time >= next_broadcast:
                await broadcast_message(f"KRIA Vbus={v:.3f}V I={i:.3f}A P={p:.3f}W")
                next_broadcast = sample_time + 1.0
        except Exception as exc:
            with _kria_power_lock:
                _kria_power_status = str(exc)
        await asyncio.sleep(0.1)


def stop_snn_feature_worker():
    global _feature_worker_proc
    with _feature_worker_lock:
        if _feature_worker_proc is not None:
            if _feature_worker_proc.poll() is None:
                _feature_worker_proc.terminate()
                try:
                    _feature_worker_proc.wait(timeout=2.0)
                except subprocess.TimeoutExpired:
                    _feature_worker_proc.kill()
            _feature_worker_proc = None

def _read_hwmon_float(base, candidates):
    for filename in candidates:
        try:
            with open(os.path.join(base, filename), "r") as f:
                return float(f.read().strip())
        except (OSError, ValueError):
            continue
    raise FileNotFoundError(f"Missing hwmon channel: {', '.join(candidates)}")


def enqueue_serial_message(outbound_queue, msg):
    if outbound_queue.full():
        try:
            outbound_queue.get_nowait()  # discard stale telemetry to keep the reader decoupled
        except asyncio.QueueEmpty:
            pass
    try:
        outbound_queue.put_nowait(msg)
    except asyncio.QueueFull:
        pass

async def serial_broadcast_worker(outbound_queue):
    while not shutdown_event.is_set():
        msg = await outbound_queue.get()
        try:
            await asyncio.wait_for(broadcast_message(msg), timeout=0.5)
        except asyncio.TimeoutError:
            pass
        except Exception as e:
            print(f"WebSocket broadcast error: {e}")

async def broadcast_message(msg):
    if clients:
        await asyncio.gather(*(client.send(msg) for client in tuple(clients)), return_exceptions=True)

async def main():
    loop = asyncio.get_running_loop()
    outbound_queue = asyncio.Queue(maxsize=512)
    t = threading.Thread(target=serial_read_loop, args=(loop, outbound_queue), daemon=True)
    writer = threading.Thread(target=usb_write_loop, daemon=True)
    t.start()
    writer.start()

    try:
        async def warm_worker_in_background():
            try:
                await asyncio.to_thread(warm_snn_feature_worker, 60.0)
                print("Python tsfresh feature worker ready")
            except Exception as exc:
                print(f"Warning: Python tsfresh worker not ready: {exc}")

        asyncio.create_task(warm_worker_in_background())
        print("WebSocket server running on ws://0.0.0.0:8080 (PyUSB mode)")
        asyncio.create_task(kria_power_loop())
        asyncio.create_task(serial_broadcast_worker(outbound_queue))
        asyncio.create_task(device_heartbeat_loop())
        async with websockets.serve(handler, "0.0.0.0", 8080):
            await asyncio.Future()
    finally:
        shutdown_event.set()
        with command_condition:
            command_condition.notify_all()
        if persister_proc:
            persister_proc.terminate()
        stop_snn_feature_worker()
        close_serial()
        t.join(timeout=2)
        writer.join(timeout=2)

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
