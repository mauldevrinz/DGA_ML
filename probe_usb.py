import usb.core
import usb.util

dev = usb.core.find(idVendor=0x16c0, idProduct=0x0483)
if dev is None:
    print("Device not found")
    exit()

print(f"Found {dev}")
for cfg in dev:
    for intf in cfg:
        print(f"Interface {intf.bInterfaceNumber}, Alt {intf.bAlternateSetting}, Class {intf.bInterfaceClass}")
        for ep in intf:
            print(f"  Endpoint: {hex(ep.bEndpointAddress)}, Type: {usb.util.endpoint_type(ep.bmAttributes)}, In/Out: {usb.util.endpoint_direction(ep.bEndpointAddress)}")
