#![no_main]
#![no_std]

extern crate panic_semihosting;  // 4004 bytes
// extern crate panic_halt; // 672 bytes

#[macro_use(block)]
extern crate nb;

use cortex_m_rt::entry;

use lpc55_hal as hal;
use hal::prelude::*;
use hal::{
    drivers::{
        Pins,
        Timer,
        Pwm,
    },
};
pub use hal::typestates::pin::state;

// translated from https://stackoverflow.com/a/2284929/2490057
fn sin(x: f32) -> f32
{

    let mut res = 0f32;
    let mut pow = x;
    let mut fact = 1f32;
    for i in 0..5 {
        res += pow/fact;
        pow *= -1f32 * x * x;
        fact *= ((2*(i+1))*(2*(i+1)+1)) as f32;
    }

    res
}

fn print_type_of<T>(_: &T) {
    use cortex_m_semihosting::{hprintln};
    hprintln!("{}", core::any::type_name::<T>()).ok();
}

#[entry]
fn main() -> ! {

    let mut hal = hal::new();

    let clocks = hal::ClockRequirements::default()
        .system_frequency(96.MHz())
        .configure(&mut hal.anactrl, &mut hal.pmc, &mut hal.syscon)
        .unwrap();

    let mut iocon = hal.iocon.enabled(&mut hal.syscon);
    let pins = Pins::take().unwrap();

    let mut delay_timer = Timer::new(hal.ctimer.0.enabled(&mut hal.syscon, clocks.support_1mhz_fro_token().unwrap()));

    // Xpresso LED (they used same channel for two pins)
    // let mut pwm = Pwm::new(hal.ctimer.2.enabled(&mut hal.syscon, clocks.support_1mhz_fro_token().unwrap()));
    // let blue = pins.pio1_6.into_match_output(&mut iocon);
    // let green = pins.pio1_7.into_match_output(&mut iocon);
    // let red = pins.pio1_4.into_match_output(&mut iocon);

    // Bee LED
    let mut pwm = Pwm::new(hal.ctimer.3.enabled(&mut hal.syscon, clocks.support_1mhz_fro_token().unwrap()));
    let red = pins.pio1_21.into_match_output(&mut iocon);
    let green = pins.pio0_5.into_match_output(&mut iocon);
    let blue = pins.pio1_19.into_match_output(&mut iocon);

    // 0 = 100% high voltage / off
    // 128 = 50% high/low voltage
    // 255 = 0% high voltage/ fully on
    pwm.set_duty(green.get_channel(), 0);
    pwm.set_duty(red.get_channel(), 0);
    pwm.set_duty(blue.get_channel(), 0);
    pwm.enable(green.get_channel());
    pwm.enable(red.get_channel());
    pwm.enable(blue.get_channel());

    print_type_of(&blue);

    let mut duties = [0f32, 30f32, 60f32];
    let increments = [0.3f32, 0.2f32, 0.1f32];

    pwm.scale_max_duty_by(10);

    loop {

        delay_timer.start(5_000.microseconds());
        block!(delay_timer.wait()).unwrap();

        for i in 0..3 {
            duties[i] += increments[i];
            if duties[i] >= 180f32 {
                duties[i] -= 180f32;
            }
        }

        for i in 0..3 {
            let duty = (sin(duties[i] * 3.14159265f32/180f32) * 255f32) as u16;
            match i {
                0 => {
                    // need to tune down red some
                    pwm.set_duty(red.get_channel(), duty as u16);
                }
                1 => {
                    pwm.set_duty(green.get_channel(), duty*2);
                }
                2 => {
                    pwm.set_duty(blue.get_channel(), duty*2);
                }
                _ => {}
            }

        }

    }
}
