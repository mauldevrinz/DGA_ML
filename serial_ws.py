#!/usr/bin/env python3
import asyncio
from collections import deque
import websockets
import usb.core
import usb.util
import threading
import queue
import multiprocessing as mp
import time
import os
import base64
import math
import struct
import numpy as np
import json
import re
import pandas as pd
from tsfresh import extract_features
from tsfresh.feature_extraction import EfficientFCParameters
from tsfresh.utilities.dataframe_functions import impute

from fuzzy_pid import FuzzyPIDController

# --- SNN FPGA inference setup ---
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PARAMS_FILE = os.path.join(SCRIPT_DIR, "fpga", "bitstream", "params.bin")
_snn_params_data = None
_snn_lock = threading.Lock()

def _load_snn_params():
    """Load SNN parameters from disk (once)."""
    global _snn_params_data
    if _snn_params_data is None:
        if os.path.exists(PARAMS_FILE):
            with open(PARAMS_FILE, "rb") as f:
                _snn_params_data = f.read()
            print(f"SNN params loaded: {len(_snn_params_data)} bytes")
        else:
            print(f"Warning: SNN params not found at {PARAMS_FILE}")
    return _snn_params_data

def _diagnose_iec60599(gas_confidences):
    """
    Rule-based IEC 60599 fault diagnosis from gas composition confidences.
    Matches logic from colab/DGA_ML_GasComposition_Training.py
    Input: dict with keys 'AIR','C2H2','C2H4','H2','CH4' (confidence 0-100)
    Returns: dict with fault_code, fault_type, description, severity
    """
    c2h2 = gas_confidences.get('C2H2', 0)
    c2h4 = gas_confidences.get('C2H4', 0)
    h2   = gas_confidences.get('H2', 0)
    ch4  = gas_confidences.get('CH4', 0)
    udara = gas_confidences.get('AIR', 0)

    SIGNIFICANT = 30

    # All gas low or clean air dominant
    if udara > 70 and c2h2 < SIGNIFICANT and c2h4 < SIGNIFICANT and h2 < SIGNIFICANT:
        return {"fault_code": "Normal", "fault_type": "No Fault",
                "description": "Tidak ada fault terdeteksi", "severity": "normal"}

    fault_total = c2h2 + c2h4 + h2
    if fault_total < 10:
        return {"fault_code": "Normal", "fault_type": "No Fault",
                "description": "Konsentrasi fault gas terlalu rendah", "severity": "normal"}

    p_c2h2 = c2h2 / fault_total
    p_c2h4 = c2h4 / fault_total
    p_h2   = h2 / fault_total
    r1 = c2h2 / max(c2h4, 1)

    # D2: High Energy Discharge (Arcing)
    if p_c2h2 > 0.40 and h2 > SIGNIFICANT:
        return {"fault_code": "D2", "fault_type": "Electrical Fault",
                "description": f"High Energy Discharge — Arcing (C2H2/C2H4={r1:.2f})",
                "severity": "critical"}

    # D1: Low Energy Discharge (Sparking)
    if p_c2h2 > 0.25 and c2h2 > SIGNIFICANT:
        return {"fault_code": "D1", "fault_type": "Electrical Fault",
                "description": f"Low Energy Discharge — Sparking (C2H2/C2H4={r1:.2f})",
                "severity": "warning"}

    # PD: Partial Discharge
    if p_h2 > 0.50 and c2h2 < SIGNIFICANT:
        return {"fault_code": "PD", "fault_type": "Electrical Fault",
                "description": f"Partial Discharge (H2={p_h2:.0%})", "severity": "caution"}

    # T3: Thermal Fault > 700°C
    if p_c2h4 > 0.60 and c2h4 > 60:
        return {"fault_code": "T3", "fault_type": "Thermal Fault",
                "description": f"Thermal Fault > 700°C (C2H4={c2h4:.0f}%)", "severity": "critical"}

    # T2: Thermal Fault 300-700°C
    if p_c2h4 > 0.40 and c2h4 > SIGNIFICANT:
        return {"fault_code": "T2", "fault_type": "Thermal Fault",
                "description": f"Thermal Fault 300–700°C (C2H4={p_c2h4:.0%})", "severity": "warning"}

    # T1: Thermal Fault < 300°C
    if p_c2h4 > 0.20 or c2h4 > SIGNIFICANT:
        return {"fault_code": "T1", "fault_type": "Thermal Fault",
                "description": "Thermal Fault < 300°C", "severity": "caution"}

    # DT: Mixed Thermal & Electrical
    if c2h2 > SIGNIFICANT and c2h4 > SIGNIFICANT:
        return {"fault_code": "DT", "fault_type": "Mixed Fault",
                "description": f"Mixed Thermal & Electrical (C2H2={c2h2:.0f}%, C2H4={c2h4:.0f}%)",
                "severity": "critical"}

    return {"fault_code": "T1", "fault_type": "Thermal Fault",
            "description": "Thermal Fault < 300°C", "severity": "caution"}


def snn_fpga_inference(sensor_values):
    """
    Run SNN inference on the FPGA with given sensor values.
    sensor_values: list of 16 float values (MOS sensor readings in mV)
    Returns: dict with classification results
    """
    try:
        # Add the fpga directory to path so we can import snn_driver
        import sys
        fpga_dir = os.path.join(SCRIPT_DIR, "fpga")
        if fpga_dir not in sys.path:
            sys.path.insert(0, fpga_dir)
        from snn_driver import run_inference

        params = _load_snn_params()
        if params is None:
            return {"ok": False, "error": "SNN params.bin not found"}

        # Pack sensor values as float32 input
        input_data = struct.pack(f"<{len(sensor_values)}f", *sensor_values)

        # Output: 5 float32 (5 DGA fault classes)
        output_size = 20

        t0 = time.monotonic()
        with _snn_lock:
            result_bytes = run_inference(input_data, params, output_size, verbose=False)
        elapsed_ms = (time.monotonic() - t0) * 1000

        # Parse output — 5 neurons = confidence per gas (matches training)
        outputs = list(struct.unpack("<5f", result_bytes))

        GAS_NAMES = ["AIR", "C2H2", "C2H4", "H2", "CH4"]

        # Build gas confidence dict (0-100 scale)
        gas_confidences = {}
        for i, name in enumerate(GAS_NAMES):
            gas_confidences[name] = round(float(outputs[i]) * 100, 1)

        # IEC 60599 rule-based fault diagnosis from gas composition
        iec_diagnosis = _diagnose_iec60599(gas_confidences)

        predicted_idx = max(range(5), key=lambda i: outputs[i])

        return {
            "ok": True,
            "outputs": outputs,
            "gas_names": GAS_NAMES,
            "gas_confidences": gas_confidences,
            "predicted_gas": GAS_NAMES[predicted_idx],
            "predicted_idx": predicted_idx,
            "confidence": outputs[predicted_idx],
            "iec_diagnosis": iec_diagnosis,
            "latency_ms": round(elapsed_ms, 2),
        }
    except Exception as e:
        import traceback
        return {"ok": False, "error": str(e), "trace": traceback.format_exc()}

setpoint = 0.0
fuzzy_controller = FuzzyPIDController(kp=5.0, ki=0.5, kd=1.0)

dev = None
usb_lock = threading.Lock()
shutdown_event = threading.Event()

import subprocess

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
    global phase_task, phase_owner, phase_ack_future
    phase_owner = websocket
    previous_cycle = None
    try:
        queue_command("SYSTEM_ARM")
        queue_command(f"PWM={pump_pwm}")
        for item in phases:
            if websocket not in clients or phase_owner is not websocket:
                break
            cycle_number = item["cycle"]
            phase = item["phase"]
            duration = item["duration_sec"]
            if cycle_number != previous_cycle:
                queue_command(f"CYCLE={cycle_number}")
                previous_cycle = cycle_number
            if phase == "INJECT":
                queue_command(f"PWM={pump_pwm}")

            loop = asyncio.get_running_loop()
            phase_ack_future = loop.create_future()
            queue_command(f"PHASE={phase}")
            try:
                acknowledged = await asyncio.wait_for(phase_ack_future, timeout=5.0)
            except asyncio.TimeoutError:
                await broadcast_message(json.dumps({
                    "type": "SYSTEM_ERROR",
                    "message": f"The Teensy did not acknowledge the {phase} phase within 5 seconds.",
                }))
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
        if phase_task is asyncio.current_task():
            phase_task = None


def _abort_system_after_watchdog():
    global phase_task, phase_owner, phase_active
    if phase_task is not None and not phase_task.done():
        phase_task.cancel()
    phase_task = None
    phase_owner = None
    with command_condition:
        phase_active = False


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


def csv_to_model_features(rows):
    feature_file = os.path.join(SCRIPT_DIR, "colab", "selected_feature_names.json")
    model_file = os.path.join(SCRIPT_DIR, "colab", "snn_gas_composition.json")
    with open(feature_file) as f: feature_names = json.load(f)["feature_names"]
    with open(model_file) as f: model = json.load(f)
    sensor_names = ["mos0_mV","mos1_mV","mos2_mV","mos3_mV","mos4_mV","mos5_mV","mos6_mV","mos7_mV","mos8_mV","mos9_mV","mos10_mV","mos11_mV","mos12_mV","mos13_mV","mos14_mV","mos15_mV","temp_chamber_C","temp_oil_C","humidity_chamber_pct","humidity_oil_pct","ina226_1_vbus_V","ina226_1_current_mA","ina226_1_power_W","ina226_2_vbus_V","ina226_2_current_mA","ina226_2_power_W","kria_vbus_V","kria_current_mA","kria_power_W","ndir_ratio","ndir_baseline","ndir_response_pct","ndir_absorbance_au","flow_inlet"]
    df = pd.DataFrame(rows)
    if df.empty or len(df) < 288:
        raise ValueError(f'CSV membutuhkan minimal 288 baris; diterima {len(df)}')
    missing = [name for name in sensor_names if name not in df.columns]
    if missing:
        raise ValueError('Kolom sensor CSV hilang: ' + ', '.join(missing))
    df = df[sensor_names].apply(pd.to_numeric, errors='coerce')
    invalid = [name for name in sensor_names if df[name].isna().any()]
    if invalid:
        raise ValueError('Nilai non-numerik/NaN pada kolom: ' + ', '.join(invalid))
    missing=[c for c in sensor_names if c not in df.columns]
    if missing: raise ValueError("CSV missing columns: " + ", ".join(missing))
    df = df[sensor_names]
    df["id"]=0; df["time"]=np.arange(len(df))
    features=extract_features(df, column_id="id", column_sort="time", default_fc_parameters=EfficientFCParameters(), disable_progressbar=True)
    impute(features)
    for name in feature_names:
        if name not in features: features[name]=0.0
    x=features[feature_names].to_numpy(dtype=np.float32)[0]
    mean=np.asarray(model["norm"]["mean"],dtype=np.float32); std=np.asarray(model["norm"]["std"],dtype=np.float32)
    if len(x)!=7530 or len(mean)!=len(x): raise ValueError(f"feature dimension mismatch: {len(x)}")
    return np.nan_to_num((x-mean)/np.where(std==0,1.0,std)).tolist()

def _snn_worker(sensor_values, result_queue):
    try:
        result_queue.put(snn_fpga_inference(sensor_values))
    except BaseException as exc:
        result_queue.put({"ok": False, "error": repr(exc)})

def snn_fpga_inference_safe(sensor_values, timeout=8.0):
    ctx=mp.get_context("fork")
    result_queue=ctx.Queue(maxsize=1)
    proc=ctx.Process(target=_snn_worker, args=(sensor_values, result_queue), daemon=True)
    proc.start()
    proc.join(timeout)
    if proc.is_alive():
        proc.terminate()
        proc.join(2.0)
        if proc.is_alive(): proc.kill()
        return {"ok": False, "error": f"FPGA inference watchdog timeout after {timeout:.1f}s"}
    if not result_queue.empty(): return result_queue.get()
    return {"ok": False, "error": f"FPGA worker exited with code {proc.exitcode}"}

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
                        phase_task = asyncio.create_task(run_system_plan(websocket, plan, pump_pwm))
                        await websocket.send(json.dumps({"type": "SYSTEM_PLAN_ACCEPTED", "phases": len(plan)}))
                        continue

                    if cmd.get('type') == 'STOP_SYSTEM':
                        if phase_task is not None and not phase_task.done():
                            phase_task.cancel()
                        phase_task = None
                        phase_owner = None
                        queue_command("PHASE=OFF")
                        await broadcast_message(json.dumps({"type": "SYSTEM_STOPPED"}))
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
                        if len(rows) < 288: raise ValueError(f"CSV needs 288 rows, got {len(rows)}")
                        request_t0 = time.monotonic()
                        feature_t0 = time.monotonic()
                        values = csv_to_model_features(rows[:288])
                        feature_latency_ms = round((time.monotonic() - feature_t0) * 1000, 2)
                        await websocket.send(json.dumps({'type':'CSV_PROGRESS','stage':'fpga','progress':85,'message':'Running FPGA inference'}))
                        loop = asyncio.get_running_loop()
                        result = await loop.run_in_executor(None, snn_fpga_inference_safe, values)
                        result['feature_latency_ms'] = feature_latency_ms
                        result['total_latency_ms'] = round((time.monotonic() - request_t0) * 1000, 2)
                        result['type'] = 'SNN_RESULT'
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
        """Drain persist_queue → persister_proc.stdin in a separate thread
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
                                loop.call_soon_threadsafe(_abort_system_after_watchdog)
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
                    dev = None
                    buffer.clear()  # Clear stale bytes to prevent frame contamination on reconnect
                    time.sleep(1)
            except Exception as e:
                if shutdown_event.is_set():
                    break
                print(f"USB disconnected or error: {e}")
                dev = None
                buffer.clear()  # Clear stale bytes to prevent frame contamination on reconnect
                time.sleep(1)
        else:
            shutdown_event.wait(3)
            connect_serial()

async def kria_power_loop():
    import glob
    while not shutdown_event.is_set():
        try:
            base = None
            for d in glob.glob("/sys/class/hwmon/hwmon*"):
                try:
                    with open(os.path.join(d, "name"), "r") as f:
                        if "ina260" in f.read():
                            base = d
                            break
                except: pass
            if base:
                with open(os.path.join(base, "in1_input"), "r") as f:
                    v = float(f.read().strip()) / 1000.0
                with open(os.path.join(base, "curr1_input"), "r") as f:
                    i = float(f.read().strip()) / 1000.0
                with open(os.path.join(base, "power1_input"), "r") as f:
                    p = float(f.read().strip()) / 1e6
                msg = f"KRIA Vbus={v:.3f}V I={i:.3f}A P={p:.3f}W"
                await broadcast_message(msg)
        except Exception:
            pass
        await asyncio.sleep(1.0)

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
        close_serial()
        t.join(timeout=2)
        writer.join(timeout=2)

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
