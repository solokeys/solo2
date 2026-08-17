//! NFC wiring: FC1 I2C on PIO0_13/PIO0_14, NC08 IRQ on PIO0_19, 082C IRQ on PIO1_22.

use crate::hal::{
    self,
    drivers::pins::{self, Pin},
    typestates::pin::{gpio::direction, state::Gpio},
    Enabled, Iocon,
};

pub type IrqPin = pins::Pio0_19;
pub type I2cIrqPin = pins::Pio1_22;
pub type Flexcomm = hal::peripherals::flexcomm::Flexcomm1<hal::typestates::init_state::Unknown>;
pub type I2cPeriph = hal::peripherals::flexcomm::I2c1<Enabled>;

pub fn set_irq_pullup(iocon: &hal::raw::iocon::RegisterBlock) {
    iocon.pio0_19.modify(|_, w| w.mode().pull_up());
}

/// True-I2C pads: FUNC1 (FC1 I2C), EGP=I2C, standard slew, 50 ns filter.
pub fn configure_i2c_pins(_iocon: &mut Iocon<Enabled>, iocon_raw: &hal::raw::iocon::RegisterBlock) {
    let _sda = pins::Pio0_13::take().unwrap();
    let _scl = pins::Pio0_14::take().unwrap();
    iocon_raw.pio0_13.modify(|_, w| {
        w.func()
            .alt1()
            .egp()
            .i2c_mode()
            .mode()
            .inactive()
            .digimode()
            .digital()
            .slew()
            .standard()
            .i2cfilter()
            .fast_mode()
    });
    iocon_raw.pio0_14.modify(|_, w| {
        w.func()
            .alt1()
            .egp()
            .i2c_mode()
            .mode()
            .inactive()
            .digimode()
            .digital()
            .slew()
            .standard()
            .i2cfilter()
            .fast_mode()
    });
}

/// The 082C IRQ is a separate pad here (P20 pin 8), open-drain active-low, so it
/// needs its own pull-up rather than reusing the NC08 IRQ.
pub fn take_i2c_irq(
    _nfc_irq: &mut Option<Pin<IrqPin, Gpio<direction::Input>>>,
    iocon: &mut Iocon<Enabled>,
    gpio: &mut hal::Gpio<Enabled>,
) -> Pin<I2cIrqPin, Gpio<direction::Input>> {
    let int = I2cIrqPin::take()
        .unwrap()
        .into_gpio_pin(iocon, gpio)
        .into_input();
    unsafe { &*hal::raw::IOCON::ptr() }
        .pio1_22
        .modify(|_, w| w.mode().pull_up());
    int
}
