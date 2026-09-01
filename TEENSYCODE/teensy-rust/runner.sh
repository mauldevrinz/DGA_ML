#!/bin/bash
rust-objcopy -O ihex -R .defmt.end "$1" "$1.hex"
sed -i 's/\r$//' "$1.hex"
teensy_loader_cli --mcu=TEENSY41 -w -v "$1.hex"