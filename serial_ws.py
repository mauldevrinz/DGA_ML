#!/usr/bin/env python3
import asyncio
import websockets
import usb.core
import usb.util
import threading
import time
import os

from persistence import SensorPersistence

dev = None
persistence = SensorPersistence()

def connect_serial():
    global dev
    try:
        new_dev = usb.core.find(idVendor=0x16c0, idProduct=0x0483)
        if new_dev is not None:
            if new_dev.is_kernel_driver_active(1):
                new_dev.detach_kernel_driver(1)
            new_dev.set_configuration()
            # Set DTR/RTS to signal terminal is connected
            new_dev.ctrl_transfer(0x21, 0x22, 0x03, 0, None)
            dev = new_dev
            print("Connected to Teensy via PyUSB")
        else:
            dev = None
    except Exception as e:
        print(f"Failed to connect to USB: {e}")
        dev = None

connect_serial()
clients = set()

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
        ax.set_ylim(bottom=0)

        # Legend outside right
        ax.legend(
            loc='upper left', bbox_to_anchor=(1.01, 1.0),
            fontsize=8, frameon=True, framealpha=0.9,
            borderpad=0.8, labelspacing=0.4
        )

        fig.tight_layout(rect=[0, 0, 0.82, 1])
        fig.savefig(output_png, dpi=120, bbox_inches='tight')
        plt.close(fig)

    return {'ok': True, 'path': output_png}


async def handler(websocket):
    clients.add(websocket)
    print(f"Client connected. Total clients: {len(clients)}")
    try:
        async for message in websocket:
            # Detect JSON commands from GUI (not serial commands)
            if message.startswith('{'):
                import json
                try:
                    cmd = json.loads(message)
                    if cmd.get('type') == 'SAVE_GRAPH':
                        result = generate_gnuplot_graph(
                            data=cmd['data'],
                            baselines=cmd['baselines'],
                            session_name=cmd['sessionName'],
                            sensor_names=cmd['sensorNames'],
                        )
                        await websocket.send(json.dumps(result))
                except Exception as e:
                    import traceback
                    await websocket.send(json.dumps({'ok': False, 'error': str(e), 'trace': traceback.format_exc()}))
                continue

            # Otherwise: forward to Teensy via USB
            print(f"Command from GUI: {message}")
            if dev is not None:
                if not message.endswith('\n'):
                    message += '\n'
                try:
                    dev.write(0x1, message.encode('utf-8'))
                except Exception as e:
                    print(f"Write error: {e}")
            else:
                print("USB not connected, ignoring command.")
    except websockets.exceptions.ConnectionClosed:
        pass
    finally:
        clients.remove(websocket)
        print("Client disconnected.")

def serial_read_loop(loop):
    global dev
    buffer = bytearray()
    while True:
        if dev is not None:
            try:
                # Read from Bulk IN endpoint (0x82)
                data = dev.read(0x82, 64, timeout=1000)
                if data:
                    buffer.extend(data)
                    # Process lines if \n is present
                    while b'\n' in buffer:
                        line, buffer = buffer.split(b'\n', 1)
                        decoded = line.decode('utf-8', errors='ignore').strip()
                        if decoded:
                            try:
                                persistence.process_line(decoded)
                            except Exception as e:
                                print(f"Persistence error: {e}")
                            if clients:
                                asyncio.run_coroutine_threadsafe(broadcast_message(decoded), loop)
            except usb.core.USBError as e:
                # Timeout is normal if no data, error code usually 110
                if e.errno == 110 or 'timeout' in str(e).lower():
                    continue
                else:
                    print(f"USB read error: {e}")
                    dev = None
                    time.sleep(1)
            except Exception as e:
                print(f"USB disconnected or error: {e}")
                dev = None
                time.sleep(1)
        else:
            time.sleep(3)
            connect_serial()

async def broadcast_message(msg):
    if clients:
        tasks = [asyncio.create_task(client.send(msg)) for client in clients]
        await asyncio.wait(tasks)

async def main():
    loop = asyncio.get_running_loop()
    t = threading.Thread(target=serial_read_loop, args=(loop,), daemon=True)
    t.start()
    
    print("WebSocket server running on ws://0.0.0.0:8080 (PyUSB mode)")
    async with websockets.serve(handler, "0.0.0.0", 8080):
        await asyncio.Future()

if __name__ == "__main__":
    asyncio.run(main())
