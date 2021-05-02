use crate::hal::{
    self,
    drivers::{
        pins::{self, Pin},
        SpiMaster,
        Timer,
    },
    Enabled,
    peripherals::flexcomm::Spi0,
    time::RateExtensions,
    typestates::{
        pin::{
            self,
            flexcomm::NoPio,
        },
    },
};

use fm11nc08::{
    FM11NC08, Configuration, Register,
};

pub type NfcSckPin = pins::Pio0_28;
pub type NfcMosiPin = pins::Pio0_24;
pub type NfcMisoPin = pins::Pio0_25;
pub type NfcCsPin = pins::Pio1_20;
pub type NfcIrqPin = pins::Pio0_19;

pub type NfcChip = FM11NC08<
            SpiMaster<
                NfcSckPin,
                NfcMosiPin,
                NfcMisoPin,
                NoPio,
                Spi0,
                (
                    Pin<NfcSckPin, pin::state::Special<pin::function::FC0_SCK>>,
                    Pin<NfcMosiPin, pin::state::Special<pin::function::FC0_RXD_SDA_MOSI_DATA>>,
                    Pin<NfcMisoPin, pin::state::Special<pin::function::FC0_TXD_SCL_MISO_WS>>,
                    pin::flexcomm::NoCs,
                )
                >,
                Pin<NfcCsPin, pin::state::Gpio<pin::gpio::direction::Output>>,
                Pin<NfcIrqPin, pin::state::Gpio<pin::gpio::direction::Input>>,
            >;

pub fn try_setup(
    spi: Spi0<Enabled>,
    gpio: &mut hal::Gpio<Enabled>,
    iocon: &mut hal::Iocon<Enabled>,
    nfc_irq: Pin<NfcIrqPin, pin::state::Gpio<pin::gpio::direction::Input>>,
    // fm: &mut NfcChip,
    timer: &mut Timer<impl hal::peripherals::ctimer::Ctimer<hal::typestates::init_state::Enabled>>,
    always_reconfig: bool,
    ) -> Result<NfcChip, ()> {


    let sck = NfcSckPin::take().unwrap().into_spi0_sck_pin(iocon);
    let mosi = NfcMosiPin::take().unwrap().into_spi0_mosi_pin(iocon);
    let miso = NfcMisoPin::take().unwrap().into_spi0_miso_pin(iocon);
    let spi_mode = hal::traits::wg::spi::Mode {
        polarity: hal::traits::wg::spi::Polarity::IdleLow,
        phase: hal::traits::wg::spi::Phase::CaptureOnSecondTransition,
    };
    let spi = SpiMaster::new(
        spi,
        (sck, mosi, miso, hal::typestates::pin::flexcomm::NoCs),
        2_000_000u32.Hz(),
        spi_mode);

    // Start unselected.
    let nfc_cs = NfcCsPin::take().unwrap().into_gpio_pin(iocon, gpio).into_output_high();

    let mut fm = FM11NC08::new(spi, nfc_cs, nfc_irq).enabled();

    //                      no limit      2mA resistor    3.3V
    const REGU_CONFIG: u8 = (0b11 << 4) | (0b10 << 2) | (0b11 << 0);
    let current_regu_config = fm.read_reg(fm11nc08::Register::ReguCfg);
    let current_nfc_config = fm.read_reg(fm11nc08::Register::NfcCfg);

    // regu_config gets configured by upstream vendor testing, so we need
    // to additionally test on another value to see if eeprom is configured by us.
    let is_select_int_masked = (current_nfc_config &  1) == 1;

    if current_regu_config == 0xff {
        // No nfc chip connected
        info!("No NFC chip connected");
        return Err(());
    }

    let reconfig = always_reconfig || (current_regu_config != REGU_CONFIG) || (is_select_int_masked);

    if reconfig {
        // info_now!("{:?}", fm.dump_eeprom() );
        // info_now!("{:?}", fm.dump_registers() );

        info!("writing EEPROM");

        let r = fm.configure(Configuration{
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
            nfc:    (0b0 << 1) |       (0b00 << 2),
        }, timer);
        if r.is_err() {
            info!("Eeprom failed.  No NFC chip connected?");
            return Err(());
        }
    } else {
        info!("EEPROM already initialized.");
    }

    // disable all interrupts except RxStart
    fm.write_reg(Register::AuxIrqMask, 0x00);
    fm.write_reg(Register::FifoIrqMask,
        // 0x0
        0xff
        ^ (1 << 3) /* water-level */
        ^ (1 << 1) /* fifo-full */
    );
    fm.write_reg(Register::MainIrqMask,
        // 0x0
        0xff
        ^ fm11nc08::device::Interrupt::RxStart as u8
        ^ fm11nc08::device::Interrupt::RxDone as u8
        ^ fm11nc08::device::Interrupt::TxDone as u8
        ^ fm11nc08::device::Interrupt::Fifo as u8
        ^ fm11nc08::device::Interrupt::Active as u8
    );

    //                    no limit    rrfcfg .      3.3V
    // let regu_powered = (0b11 << 4) | (0b10 << 2) | (0b11 << 0);
    // fm.write_reg(Register::ReguCfg, regu_powered);

    Ok(fm)
}

