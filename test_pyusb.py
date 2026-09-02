import usb.core
import usb.util
import time

dev = usb.core.find(idVendor=0x16c0, idProduct=0x0483)
if dev is None:
    print("Device not found")
    exit()

if dev.is_kernel_driver_active(1):
    dev.detach_kernel_driver(1)

dev.set_configuration()

# Send DTR/RTS to start data (Teensy sometimes requires this to enable serial output)
# CDC ACM Set_Control_Line_State (DTR=1, RTS=1)
dev.ctrl_transfer(0x21, 0x22, 0x03, 0, None)

print("Reading...")
try:
    data = dev.read(0x82, 64, timeout=1000)
    print("Read data:", data.tobytes().decode('utf-8', errors='ignore'))
except usb.core.USBError as e:
    print("USBError:", e)
