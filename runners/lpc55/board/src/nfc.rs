use crate::hal::{
    self,
    drivers::{
        pins::{self, Pin},
        Timer,
    },
    typestates::pin,
    Enabled,
};

use defmt::info;

use fm11nc08::{Configuration, Register, FM11NC08};

pub type NfcCsPin = pins::Pio1_20;
pub type NfcIrqPin = pins::Pio0_19;

pub type NfcChip = FM11NC08<
    crate::shared_spi::BusProxy,
    crate::shared_spi::LockCs<Pin<NfcCsPin, pin::state::Gpio<pin::gpio::direction::Output>>>,
    Pin<NfcIrqPin, pin::state::Gpio<pin::gpio::direction::Input>>,
>;

pub fn try_setup(
    gpio: &mut hal::Gpio<Enabled>,
    iocon: &mut hal::Iocon<Enabled>,
    nfc_irq: Pin<NfcIrqPin, pin::state::Gpio<pin::gpio::direction::Input>>,
    timer: &mut Timer<impl hal::peripherals::ctimer::Ctimer<hal::typestates::init_state::Enabled>>,
    always_reconfig: bool,
) -> Option<NfcChip> {
    // The shared Spi0 bus must already be installed via `shared_spi::setup`.
    // The CS is wrapped in `LockCs` (mode 1 / CPHA=1 for the FM11NC08) so each
    // NFC transaction is atomic against a concurrent flash transaction.
    // Start unselected.
    // `true` = SPI mode 1 (CPHA=1), required by the FM11NC08.
    let nfc_cs = crate::shared_spi::LockCs::new(
        NfcCsPin::take()
            .unwrap()
            .into_gpio_pin(iocon, gpio)
            .into_output_high(),
        true,
    );

    let mut fm = FM11NC08::new(crate::shared_spi::BusProxy, nfc_cs, nfc_irq).enabled();

    //                      no limit      2mA resistor    3.3V
    const REGU_CONFIG: u8 = (0b11 << 4) | (0b10 << 2) | 0b11;
    let current_regu_config = fm.read_reg(fm11nc08::Register::ReguCfg);
    let current_nfc_config = fm.read_reg(fm11nc08::Register::NfcCfg);

    // regu_config gets configured by upstream vendor testing, so we need
    // to additionally test on another value to see if eeprom is configured by us.
    let is_select_int_masked = (current_nfc_config & 1) == 1;

    if current_regu_config == 0xff {
        // No nfc chip connected
        info!("No NFC chip connected");
        return None;
    }

    let reconfig =
        always_reconfig || (current_regu_config != REGU_CONFIG) || (is_select_int_masked);

    if reconfig {
        // info_now!("{:?}", fm.dump_eeprom() );
        // info_now!("{:?}", fm.dump_registers() );

        info!("writing EEPROM");

        let r = fm.configure(
            Configuration {
                regu: REGU_CONFIG,
                ataq: 0x4400,
                sak1: 0x04,
                sak2: 0x20,
                tl: 0x05,
                // (x[7:4], FSDI[3:0]) . FSDI[2] == 32 byte frame, FSDI[8] == 256 byte frame, 7==128byte
                t0: 0x78,
                // Support different data rates for both directions
                // Support divisor 2 / 212kbps for tx and rx
                ta: 0b10010001,
                // (FWI[b4], SFGI[b4]), (256 * 16 / fc) * 2 ^ value
                tb: 0x78,
                tc: 0x00,
                // enable P-on IRQ    14443-4 mode
                nfc: (0b00 << 2),
            },
            timer,
        );
        if r.is_err() {
            info!("Eeprom failed.  No NFC chip connected?");
            return None;
        }
    } else {
        info!("EEPROM already initialized.");
    }

    // disable all interrupts except RxStart
    fm.write_reg(Register::AuxIrqMask, 0x00);
    fm.write_reg(
        Register::FifoIrqMask,
        // 0x0
        0xff
        ^ (1 << 3) /* water-level */
        ^ (1 << 1), /* fifo-full */
    );
    // Only fire on RxDone (+ TxDone, Fifo) — NOT Active/RxStart. Under USB power
    // (VCC), reading MainIrq mid-reception resets the contactless RX state via
    // contact/contactless arbitration, so firing on Active/RxStart made the
    // firmware poll *during* RX and self-reset in a tight loop (MainIrq always
    // 0x00, I-block never received). Firing only on RxDone lets reception finish
    // before the first SPI read. Passive mode is unaffected (RxDone still fires).
    fm.write_reg(
        Register::MainIrqMask,
        0xff ^ fm11nc08::device::Interrupt::RxDone as u8
            ^ fm11nc08::device::Interrupt::TxDone as u8
            ^ fm11nc08::device::Interrupt::Fifo as u8,
    );

    //                    no limit    rrfcfg .      3.3V
    // let regu_powered = (0b11 << 4) | (0b10 << 2) | (0b11 << 0);
    // fm.write_reg(Register::ReguCfg, regu_powered);

    Some(fm)
}
