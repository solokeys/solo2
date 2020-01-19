use crate::hal::{
    self,
	Gpio,
	Iocon,
    Pin,
    drivers::pins,
    // traits::wg::digital::v2::OutputPin,
    typestates::{
        init_state::Enabled,
        pin::{
            self,
            state::Unused,
        },
    },
};

pub enum Color {
    Red,
    Green,
    Blue,
}

type RedLed = hal::Pin<pins::Pio1_6, pin::state::Gpio<pin::gpio::direction::Output>>;
type GreenLed = hal::Pin<pins::Pio1_7, pin::state::Gpio<pin::gpio::direction::Output>>;
type BlueLed = hal::Pin<pins::Pio1_4, pin::state::Gpio<pin::gpio::direction::Output>>;

pub struct Rgb {
    pub red: RedLed,
    pub green: GreenLed,
    pub blue: BlueLed,
}

pub fn init_leds//<S1, S2, S3>
(
    pio1_4: Pin<pins::Pio1_4, Unused>,
    pio1_6: Pin<pins::Pio1_6, Unused>,
    pio1_7: Pin<pins::Pio1_7, Unused>,
    iocon: &mut Iocon<Enabled>,
    gpio: &mut Gpio<Enabled>,
)
    -> Rgb
// where
//     S1: pin::state::Unused,
//     S2: pin::state::Unused,
//     S3: pin::state::Unused,
{

    let red = pio1_6
        .into_gpio_pin(iocon, gpio)
        .into_output(pins::Level::High)
    ;

    let green = pio1_7
        .into_gpio_pin(iocon, gpio)
        .into_output(pins::Level::High)
    ;

    let blue = pio1_4
        .into_gpio_pin(iocon, gpio)
        .into_output(pins::Level::High)
    ;

    Rgb { red, green, blue }
}

