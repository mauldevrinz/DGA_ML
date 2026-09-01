#![no_std]
#![no_main]
use teensy4_bsp as bsp;
use teensy4_panic as _;
use bsp::board;
use embedded_hal::digital::v2::OutputPin;

#[bsp::rt::entry]
fn main() -> ! {
    let board::Resources { pins, .. } = bsp::board::t41(bsp::board::instances());
    let mut p15 = bsp::hal::gpio::GPIO::new(pins.p15);
    p15.set_direction(bsp::hal::gpio::Direction::Output);
    p15.set_high();
    loop {}
}
