#!/usr/bin/env python3
import asyncio
import websockets
import usb.core
import usb.util
import threading
import time

dev = None

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

async def handler(websocket):
    clients.add(websocket)
    print(f"Client connected. Total clients: {len(clients)}")
    try:
        async for message in websocket:
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
                        if decoded and clients:
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
