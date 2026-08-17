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

use crate::nfc_i2c::BoundedI2c;
use fm11nt082c::FM11NT082C;
use nfc_device::traits::nfc;

pub type NfcCsPin = pins::Pio1_20;
pub type NfcIrqPin = crate::specifics::nfc::IrqPin;

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
        // SAFETY / why `steal` not `take`: exactly one NFC frontend is ever live. The 082C
        // autodetect (`nfc_i2c`) `take`s PIO1_20 as its I2C SCL; when no 082C answers and
        // we fall back to the NC08, that I2C is dead and the NC08 reclaims PIO1_20 as its
        // chip-select. A `take().unwrap()` here would hit `None` (pin already taken) and
        // panic during early init -> `panic_halt` -> a real Solo 2 bricks dark before USB
        // (there is no recovery). `steal` cannot fail; the pad is re-driven as a GPIO
        // output below, disconnecting the dead Flexcomm4 SCL.
        unsafe { NfcCsPin::steal() }
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

// ─── FM11NT082C (I2C) frontend ─────────────────────────────────────────────────

/// The FM11 IRQ pin (open-drain, active-low). A PINT on it (Slot0 -> the nfc_irq task)
/// drives reads on RxDone. EVK: P20 pin 8 = PIO1_22. Solo board: PIO0_19.
pub type NfcI2cIrqPin = crate::specifics::nfc::I2cIrqPin;

pub type Nfc082cChip =
    FM11NT082C<BoundedI2c, Pin<NfcI2cIrqPin, pin::state::Gpio<pin::gpio::direction::Input>>>;

/// Runtime-selected NFC frontend for ONE universal firmware: the new FM11NT082C (I2C)
/// or the legacy FM11NC08 (SPI), whichever the board has. Delegates the `nfc::Device`
/// contract to the active driver so the ISO14443/apdu stack above is chip-agnostic.
pub enum NfcFrontend {
    Fm11nc08(NfcChip),
    Fm11nt082c(Nfc082cChip),
}

impl nfc::Device for NfcFrontend {
    fn read(&mut self, buf: &mut [u8]) -> Result<nfc::State, nfc::Error> {
        match self {
            NfcFrontend::Fm11nc08(c) => c.read(buf),
            NfcFrontend::Fm11nt082c(c) => c.read(buf),
        }
    }
    fn send(&mut self, buf: &[u8]) -> Result<(), nfc::Error> {
        match self {
            NfcFrontend::Fm11nc08(c) => c.send(buf),
            NfcFrontend::Fm11nt082c(c) => c.send(buf),
        }
    }
    fn frame_size(&self) -> usize {
        match self {
            NfcFrontend::Fm11nc08(c) => c.frame_size(),
            NfcFrontend::Fm11nt082c(c) => c.frame_size(),
        }
    }
}

/// Detect + configure the FM11NT082C on the board's I2C bus. Returns None if no chip
/// ACKs (so the caller falls back to the FM11NC08). The NC/ISO14443-4 config is written
/// to EEPROM and applied live by a soft-reset inside `configure()`.
pub fn try_setup_082c(
    i2c: BoundedI2c,
    int: Pin<NfcI2cIrqPin, pin::state::Gpio<pin::gpio::direction::Input>>,
    timer: &mut Timer<impl hal::peripherals::ctimer::Ctimer<hal::typestates::init_state::Enabled>>,
) -> Option<Nfc082cChip> {
    let mut fm = FM11NT082C::new(i2c, fm11nt082c::DEFAULT_ADDR, int).enabled();

    if !fm.is_present() {
        info!("No FM11NT082C on I2C");
        return None;
    }
    info!("FM11NT082C present");

    // NC-mode + ISO14443-4 config (contact-side EEPROM write, no Fudan auth), applied by
    // the soft-reset inside configure():
    //   CFG0 0x91 (OP_MODE_SELECT=NC), CFG1 0x82 (NFC_mode[3:2]=00=ISO14443-4),
    //   CFG2 0x98 (FCFS arbitration bits[5:4]=01 + never-sleep bits[3:0]=0x8);
    //   ATQA 0x0044, SAK T4T 0x20, ATS from the FM11NC08 setup.
    let _ = fm.configure(
        fm11nt082c::Configuration {
            user_cfg0: 0x91,
            user_cfg1: 0x82,
            user_cfg2: 0x98,
            atqa: 0x0044,
            sak1: 0x00,
            sak2: 0x20,
            tl: 0x05,
            t0: 0x78,
            ta: 0b1001_0001,
            tb: 0x78,
            tc: 0x00,
        },
        timer,
    );

    fm.arm_interrupts();
    Some(fm)
}
