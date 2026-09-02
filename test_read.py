import usb.core
import time

dev = usb.core.find(idVendor=0x16c0, idProduct=0x0483)
if dev is None:
    print("Device not found")
    exit()

if dev.is_kernel_driver_active(1):
    dev.detach_kernel_driver(1)
dev.set_configuration()
dev.ctrl_transfer(0x21, 0x22, 0x03, 0, None)

end_time = time.time() + 5
while time.time() < end_time:
    try:
        data = dev.read(0x82, 64, timeout=1000)
        print(data.tobytes().decode('utf-8', errors='ignore'), end='')
    except Exception as e:
        pass
