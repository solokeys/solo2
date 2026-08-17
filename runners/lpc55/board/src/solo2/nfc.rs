//! NFC wiring: FC4 I2C on PIO1_9/PIO1_20, IRQ on PIO0_19.

use crate::hal::{
    self,
    drivers::pins::{self, Pin},
    typestates::pin::{gpio::direction, state::Gpio},
    Enabled, Iocon,
};

pub type IrqPin = pins::Pio0_19;
pub type I2cIrqPin = pins::Pio0_19;
pub type Flexcomm = hal::peripherals::flexcomm::Flexcomm4<hal::typestates::init_state::Unknown>;
pub type I2cPeriph = hal::peripherals::flexcomm::I2c4<Enabled>;

pub fn set_irq_pullup(iocon: &hal::raw::iocon::RegisterBlock) {
    iocon.pio0_19.modify(|_, w| w.mode().pull_up());
}

/// FC4 on normal pads: typed conversion sets FUNC, then force digital + open-drain.
pub fn configure_i2c_pins(iocon: &mut Iocon<Enabled>, iocon_raw: &hal::raw::iocon::RegisterBlock) {
    let _sda = pins::Pio1_9::take().unwrap().into_i2c4_sda_pin(iocon);
    let _scl = pins::Pio1_20::take().unwrap().into_i2c4_scl_pin(iocon);
    iocon_raw
        .pio1_9
        .modify(|_, w| w.digimode().digital().od().open_drain());
    iocon_raw
        .pio1_20
        .modify(|_, w| w.digimode().digital().od().open_drain());
}

pub fn take_i2c_irq(
    nfc_irq: &mut Option<Pin<IrqPin, Gpio<direction::Input>>>,
    _iocon: &mut Iocon<Enabled>,
    _gpio: &mut hal::Gpio<Enabled>,
) -> Pin<I2cIrqPin, Gpio<direction::Input>> {
    nfc_irq.take().unwrap()
}
