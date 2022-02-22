#![no_main]
#![no_std]

extern crate panic_semihosting;  // 4004 bytes
// extern crate panic_halt; // 672 bytes

#[macro_use(block)]
extern crate nb;

use cortex_m_rt::entry;
// use cortex_m_semihosting::dbg;
// use cortex_m_semihosting::heprintln;

use lpc55_hal as hal;
use hal::prelude::*;
use hal::{
    drivers::{
        Pins,
        Timer,
        touch::{
            TouchSensorChannel,
            TouchSensor,
            ButtonPins,
            Edge,
            profile_touch_sensing,
        }
    },
};
use hal::drivers::pins::Level;
pub use hal::typestates::pin::state;


#[entry]
fn main() -> ! {

    let mut hal = hal::new();

    let clocks = hal::ClockRequirements::default()
        .system_frequency(96.MHz())
        .configure(&mut hal.anactrl, &mut hal.pmc, &mut hal.syscon)
        .unwrap();
    let fro_token = clocks.support_1mhz_fro_token().unwrap();

    let touch_token = clocks.support_touch_token().unwrap();

    let mut gpio = hal.gpio.enabled(&mut hal.syscon);


    let mut iocon = hal.iocon.enabled(&mut hal.syscon);
    let pins = Pins::take().unwrap();

    let but1 = pins.pio0_23.into_analog_input(&mut iocon, &mut gpio);       // channel 0
    let but2 = pins.pio0_31.into_analog_input(&mut iocon, &mut gpio);       // channel 3
    let but3 = pins.pio0_15.into_analog_input(&mut iocon, &mut gpio);       // channel 2

    let mut green = pins
        .pio0_5
        .into_gpio_pin(&mut iocon, &mut gpio)
        .into_output(Level::High);
    let mut red = pins
        .pio1_19
        .into_gpio_pin(&mut iocon, &mut gpio)
        .into_output(Level::High);
    let mut blue = pins
        .pio1_21
        .into_gpio_pin(&mut iocon, &mut gpio)
        .into_output(Level::High);

    let mut delay_timer = Timer::new(hal.ctimer.0.enabled(&mut hal.syscon, fro_token));

    let button_pins = ButtonPins(but1,but2,but3);

    let adc = hal::Adc::from(hal.adc).enabled(&mut hal.pmc, &mut hal.syscon);

    let touch_timer = hal.ctimer.1.enabled(&mut hal.syscon, clocks.support_1mhz_fro_token().unwrap());
    let touch_sync_timer = hal.ctimer.2.enabled(&mut hal.syscon, clocks.support_1mhz_fro_token().unwrap());
    let charge_pin = pins.pio1_16.into_match_output(&mut iocon);

    let mut dma = hal::Dma::from(hal.dma).enabled(&mut hal.syscon);

    let touch_sensor = TouchSensor::new([
        13_900,
        13_900,
        13_900,
        ], 5, adc, touch_timer, touch_sync_timer, charge_pin, button_pins);
    let mut touch_sensor = touch_sensor.enabled(&mut dma, touch_token);

    // Used to get tunning information for capacitive touch
    if 1 == 1 {
        let mut counts = [0u32; 3];
        let mut times = [0u32; 128];
        let mut results = [0u32; 128];
        profile_touch_sensing(&mut touch_sensor, &mut delay_timer, &mut results, &mut times );
        for i in 0 .. 125 {
            let src = (results[i] & (0xf << 24)) >> 24;
            let _sample_num = (times[i] - 1196)/802;
            let _button_sample_num = (times[i] - 1192)/(802* 3);
            // heprintln!("{}",src).unwrap();
            // heprintln!("{}\t{}\t{}\t{}\t{}\t{}",times[i], i, sample_num,src, counts[(src-3) as usize], button_sample_num).unwrap();

            counts[(src - 3) as usize] += 1;
        }
    }

    delay_timer.start(300_000.microseconds());
    block!(delay_timer.wait()).unwrap();

    loop {

        // Check for a press
        if touch_sensor.has_edge(TouchSensorChannel::Channel1, Edge::Falling) {
            touch_sensor.reset_results(TouchSensorChannel::Channel1, -1);
            red.set_low().unwrap();
        }

        if touch_sensor.has_edge(TouchSensorChannel::Channel2, Edge::Falling) {
            touch_sensor.reset_results(TouchSensorChannel::Channel2, -1);
            green.set_low().unwrap();
        }

        if touch_sensor.has_edge(TouchSensorChannel::Channel3, Edge::Falling) {
            touch_sensor.reset_results(TouchSensorChannel::Channel3, -1);
            blue.set_low().unwrap();
        }



        // Check for a release
        if touch_sensor.has_edge(TouchSensorChannel::Channel1, Edge::Rising) {
            touch_sensor.reset_results(TouchSensorChannel::Channel1, 1);
            red.set_high().unwrap();
        }

        if touch_sensor.has_edge(TouchSensorChannel::Channel2, Edge::Rising) {
            touch_sensor.reset_results(TouchSensorChannel::Channel2, 1);
            green.set_high().unwrap();
        }

        if touch_sensor.has_edge(TouchSensorChannel::Channel3, Edge::Rising) {
            touch_sensor.reset_results(TouchSensorChannel::Channel3, 1);
            blue.set_high().unwrap();
        }

    }
}
