//! The central type is the `Fm11Nc08S` struct, which represents the FM11NC08S NFC Channel Chip.

use embedded_hal::{
    spi::FullDuplex,
    digital::v2::InputPin,
    digital::v2::OutputPin,
    timer::CountDown,
};
use nb::block;

use logging::hex::*;
use logging::hex;

use crate::logger::info;
use nfc_device::chip as nfc_chip;

pub mod types;
pub use types::*;

/// Delay used to wait for EEPROM writes to take effect (10 milliseconds).
pub const EEPROM_WRITE_DELAY: core::time::Duration = core::time::Duration::from_millis(10);

// here: mode = Mode, addr = Register
macro_rules! FM11_CMD {
    ($mode:expr, $addr:expr) => {
        match $mode {
            Mode::WriteEeprom | Mode::ReadEeprom => {
                (( ($mode as u8) & 0x07) << 5) | (((($addr as u16) & 0x300) >> 8) as u8)
            }
            _ => {
                (( ($mode as u8) & 0x07) << 5) | (($addr as u8) & 0x0f)
            }
        }
    }
}

pub mod configuration_state {
    pub trait ConfigurationState {}

    /// Typestate indicating chip has not been inspected for configuration nor explicitly configured.
    pub struct Unknown;
    impl ConfigurationState for Unknown {}
    /// Typestate indicating chip is known to be configured.
    pub struct Configured;
    impl ConfigurationState for Configured {}
}
use configuration_state::*;

/// The [FM11NC08S][fm-chip-url] chip
///
/// After [construction](struct.Fm11Nc08S.html#method.new), the chip is in `Unknown` state, and needs to be
/// [configure](struct.Fm11Nc08S.html#method.configure)d for most functionality to be allowed.
///
/// Once configured, the chip implements the high-level `nfc_device::chip::ChipDriver` trait.
///
/// ### Note on timers
///
/// Methods that write to the EEPROM (notably, chip configuration) require a timer,
/// which is used to respect the delay until writes take effect.
/// This timer needs to implement the `CountDown` timer trait from the
/// `embedded-hal`, and its associated time type needs to be constructible from the core
/// `time::Duration` type.
///
/// [fm-chip-url]: http://www.fm-chips.com/nfc-channel-ics.html
///
/*
/// ```
/// # use embedded_hal_mock::spi::Mock as Spi;
/// # use embedded_hal_mock::pin::Mock as Pin;
/// # use embedded_hal::digital::v2::{InputPin, OutputPin};
/// let spi = Spi::new(&[]);
/// let chip_select = Pin::new(&[]);
/// let interrupt = Pin::new(&[]);
///
/// use fm11nc08::{Chip, Configuration};
/// let chip = fm11nc08::Chip::new(spi, chip_select, interrupt);
/// let config = fm11nc08::Configuration { };
/// chip.configure(config, true, );
        config: Configuration,
        force_reconfiguration: bool,
        timer: &mut T,
/// ```
*/
pub struct Fm11Nc08S <SPI, CS, INT, STATE>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
    STATE: ConfigurationState,
{
    spi: SPI,
    cs: CS,
    /// interrupt PIN, currently directly exposed
    pub int: INT,
    __: core::marker::PhantomData<STATE>,
    packet: [u8; 256],
    offset: usize,
    current_frame_size: usize,
}

fn fsdi_to_frame_size(fsdi: u8) -> usize {
    match fsdi {
        0 => 16,
        1 => 24,
        2 => 32,
        3 => 40,
        4 => 48,
        5 => 64,
        6 => 96,
        7 => 128,
        _ => 256,
    }
}


impl<SPI, CS, INT> Fm11Nc08S <SPI, CS, INT, Unknown>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
{
    pub fn new(spi: SPI, cs: CS, int: INT) -> Self {
        Fm11Nc08S {
            spi,
            cs,
            int,
            __: core::marker::PhantomData,
            packet: [0u8; 256],
            offset: 0usize,
            current_frame_size: 128,
        }
    }

}

fn send<SPI: FullDuplex<u8>>(spi: &mut SPI, data: &[u8]) -> core::result::Result<(), SPI::Error> {
    // for byte in data {
    //     block!(spi.send(*byte))?;
    // }
    // for _ in 0..data.len() {
    //     block!(spi.read())?;
    // }

    for byte in data {
        block!(spi.send(*byte))?;
        block!(spi.read())?;
    }

    Ok(())
}

fn query<SPI: FullDuplex<u8>>(spi: &mut SPI, data: &[u8]) -> core::result::Result<u8, SPI::Error> {
    assert!(data.len() > 0);
    for byte in data {
        block!(spi.send(*byte))?;
    }
    for _ in 0..data.len() - 1 {
        block!(spi.read())?;
    }
    block!(spi.read())
}

impl<SPI, CS, INT, ANY> Fm11Nc08S <SPI, CS, INT, ANY>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
    ANY: ConfigurationState,
{
    /// Configure the chip.
    pub fn configure<T>(
        mut self,
        config: Configuration,
        force_reconfiguration: bool,
        timer: &mut T,
    )
    ->
        Option<Fm11Nc08S<SPI, CS, INT, Configured>>
    where
        T: CountDown,
        T::Time: Copy + From<core::time::Duration>,
    {
        let current_regulator_config = self.read_reg(Register::RegulatorConfig);

        if current_regulator_config == 0xff {
            // No nfc chip connected
            return None;
        }

        let reconfig = (current_regulator_config != config.regu) || force_reconfiguration;
        if reconfig {
            info!("{}", self.dump_configuration() ).ok();
            info!("{}", self.dump_registers() ).ok();

            info!("writing EEPROM").ok();

            self.configure_eeprom(config, timer);
        } else {
            info!("EEPROM already initialized.").ok();
        }

        self.configure_interrupts();

        Some(Fm11Nc08S {
            spi: self.spi,
            cs: self.cs,
            int: self.int,
            __: core::marker::PhantomData,
            packet: self.packet,
            offset: self.offset,
            current_frame_size: self.current_frame_size
        })
    }

    /// Deconstruct the chip driver, returning the owned
    /// SPI driver, and the chip select and interrupt pins.
    pub fn release(self) -> (SPI, CS, INT) {
        (self.spi, self.cs, self.int)
    }

    fn configure_interrupts(&mut self) {
        // I believe this configuration is necessary for the driver logic
        // to work, so we hide the detailed configuration options.
        self.configure_interrupts_detailed(
            Interrupt::RX_START |
            Interrupt::RX_DONE |
            Interrupt::TX_DONE |
            Interrupt::FIFO |
            Interrupt::ACTIVE,

            FifoInterrupt::FULL |
            FifoInterrupt::WATER_LEVEL,
        );
    }

    fn configure_interrupts_detailed(&mut self, enabled_main_interrupts: Interrupt, enabled_fifo_interrupts: FifoInterrupt) {
        // reset value: all zeros = all interrupt sources enabled
        //
        // writing 1 means masking an interrupt source,
        // so active are:
        // - all auxiliary interrupt sources
        // - the listed FIFO and main interrupt sources

        self.write_reg(Register::AuxIrqMask, 0x00);
        self.write_reg(Register::FifoIrqMask, !enabled_fifo_interrupts.bits());
        self.write_reg(Register::MainIrqMask, !enabled_main_interrupts.bits());

        //                    no limit    rrfcfg .      3.3V
        // let regulator_powered = (0b11 << 4) | (0b10 << 2) | (0b11 << 0);
        // fm.write_reg(Register::RegulatorConfig, regulator_powered);
    }

    /// Wraps interactions with the NFC chip via SPI with
    /// pulling the chip select line low. Keeps track of errors
    /// and allows using `?` in the closure.
    ///
    /// See `write_reg` for an example.
    #[inline]
    fn transact<T>(&mut self,

        f: impl FnOnce(&mut SPI) -> core::result::Result<T, SPI::Error>

    ) -> core::result::Result<T, SPI::Error> {

        // setting the CS pin low/high is infallible
        self.cs.set_low().ok();
        let result = f(&mut self.spi);
        self.cs.set_high().ok();
        result
    }

}

impl<SPI, CS, INT> Fm11Nc08S <SPI, CS, INT, configuration_state::Configured>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
{
    // pub fn fifo_irq(&mut self) -> FifoInterrupt {
    //     FifoInterrupt::from_bits_truncate(self.read_reg(Register::FifoIrq));
    // }

    /// Inherent method implementing `read` method of the `ChipDriver` trait.
    pub fn read_nfc(&mut self, buf: &mut [u8]) -> Result<nfc_chip::State, nfc_chip::Error>{

        let main_irq = self.main_irq();
        let mut new_session = false;

        if main_irq.contains(Interrupt::TX_DONE) {
            // Need to turn off transmit mode
            let count = self.read_reg(Register::FifoCount);
            info!("off transmit (-{}) {:X}", count, main_irq).ok();
        }

        let fifo_irq = if main_irq.contains(Interrupt::FIFO) {
            FifoInterrupt::from_bits_truncate(self.read_reg(Register::FifoIrq))
        } else {
            FifoInterrupt::empty()
        };

        let aux_irq = if main_irq.contains(Interrupt::AUX) {
            self.read_reg(Register::AuxIrq)
        } else {
            0
        };

        // check for overflow
        if fifo_irq.contains(FifoInterrupt::OVERFLOW) {
            // TODO: since we removed actual HAL from dependencies, no longer have
            // access to cycle count in this way. If needed, can use the implementation
            // from `cortex-m` itself, as it's just a wrapped Cortex-M / DWT method.
            //
            // info!("!OF! {} @{}", self.read_reg(Register::FifoCount), hal::get_cycle_count()/96_00).ok();
            info!("!OF! {}", self.read_reg(Register::FifoCount)).ok();
            // info!("{:X} {:X} {:X}", main_irq, fifo_irq, aux_irq).ok();
            info!("{:?} {:?} {:?}", main_irq, fifo_irq, aux_irq).ok();

            // self.write_reg(Register::FifoFlush, 0xff);
        }

        if main_irq.contains(Interrupt::ACTIVE) {
            self.offset = 0;
            new_session = true;
        }

        if main_irq.contains(Interrupt::RX_START) {
            self.offset = 0;
            let rf_rats = self.read_reg(Register::RfRats);
            self.current_frame_size = fsdi_to_frame_size((rf_rats >> 4) & 0xf);
            info!("RxStart {}", self.current_frame_size).ok();
        }

        if main_irq.contains(Interrupt::RX_DONE) {
            let count = self.read_reg(Register::FifoCount) as _;
            if count > 0 {
                self.read_fifo(count);
                self.offset += count as usize;
            }

            if self.offset <= 2 {
                // too few bytes, ignore..
                self.offset = 0;
            }
            else {
                info!("RxDone").ok();
                let l = self.offset - 2;
                for i in 0 .. l {
                    buf[i] = self.packet[i];
                }
                self.offset = 0;
                if new_session {
                    return Ok(nfc_chip::State::NewSession(l as u8));
                } else {
                    return Ok(nfc_chip::State::Continue(l as u8));
                }
            }
        }

            /* water level */
        // let rf_status = TransceptionStatus::from_bits_truncate(self.read_reg(Register::RfStatus));
        // if (fifo_irq & (1 << 3) != 0) && (rf_status & (1 << 0)) == 0 {
        if fifo_irq.contains(FifoInterrupt::WATER_LEVEL) && !self.is_transmitting() {
            let count = self.read_reg(Register::FifoCount) as _;
            info!("WL {}", count).ok();
            self.read_fifo(count);
            logging::dump_hex(&self.packet[self.offset ..], count as usize).ok();
            self.offset += count as usize;
            if count == 32 {
                info!("warning: potential ovflw").ok();
            }
        }

        // info!("{:X} {:X} {:X}", main_irq, fifo_irq, aux_irq).ok();
        info!("{:?} {:?} {:?}", main_irq, fifo_irq, aux_irq).ok();

        if new_session {
            Err(nfc_chip::Error::NewSession)
        } else {
            Err(nfc_chip::Error::NoActivity)
        }

    }

    fn wait_for_transmission(&mut self) -> Result<(), ()>{
        let mut i = 0;

        self.write_reg(Register::RfTxEn, 0x55);

        while !self.is_transmitting() {
            i += 1;
            if i > 100 {
                info!("Chip is not transmitting.").ok();
                break;
            }
        }

        let initial_count = self.read_reg(Register::FifoCount);
        let mut current_count = initial_count;
        if current_count >= 8 {

            let mut fifo_irq = FifoInterrupt::from_bits_truncate(self.read_reg(Register::FifoIrq));
            // if (rf_status & 1) == 1 {
            if self.is_transmitting() {

                // while fifo_irq & (FifoInterrupt::WATER_LEVEL as u8)) == 0 {
                while !fifo_irq.contains(FifoInterrupt::WATER_LEVEL) {
                    i += 1;
                    if i > 300 {
                        info!("TX transmission timeout.").ok();
                        break;
                    }
                    fifo_irq = FifoInterrupt::from_bits_truncate(self.read_reg(Register::FifoIrq));
                }
            }

            current_count = self.read_reg(Register::FifoCount);
            let aux_irq = self.read_reg(Register::AuxIrq);
            let rf_status = self.read_reg(Register::RfStatus);
            info!("tx {}->{}. {:X} {:X} {:X}",
                initial_count,
                current_count,
                rf_status, aux_irq, fifo_irq,
            ).ok();

            // if (fifo_irq & (FifoInterrupt::WATER_LEVEL as u8)) != 0 {
            if fifo_irq.contains(FifoInterrupt::WATER_LEVEL) {
                return Ok(())
            } else {
                return Err(())
            }
        }
        Ok(())
    }

    /// Inherent method implementing `send` method of the `ChipDriver` trait.
    pub fn send_nfc(&mut self, buf: &[u8]) -> Result<(), nfc_chip::Error>{

        // Write in chunks of 24
        for chunk in buf.chunks(24) {
            self.write_fifo(chunk);
            self.wait_for_transmission().map_err(|_| nfc_chip::Error::NoActivity)?;
        }

        Ok(())

    }

}

impl<SPI, CS, INT> nfc_chip::ChipDriver for Fm11Nc08S <SPI, CS, INT, configuration_state::Configured>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<nfc_chip::State, nfc_chip::Error>{
        self.read_nfc(buf)
    }

    fn send(&mut self,buf: &[u8]) -> Result<(), nfc_chip::Error>{
        self.send_nfc(buf)
    }

    fn frame_size(&self) -> usize {
        self.current_frame_size
    }

    // fn wait(&mut self) -> nb::Result<(), NfcError> {
        // self.wait_for_transmission_completion();
        // Ok(())
        // let main_irq = self.read_reg(Register::MainIrq);
        // if (main_irq & (Interrupt::TxDone as u8)) != 0 {
        //     // info!("wait is over. {}", logging::hex!(main_irq));
        //     self.write_reg(Register::RfTxEn, 0x00);
        //     Ok(())
        // } else {
        //     Err(nb::Error::WouldBlock)
        // }
    // }

}



impl ufmt::uDisplay for FullConfiguration {
    fn fmt<W: ?Sized>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite
    {
        use ufmt::uwriteln;
        uwriteln!(f, "").ok();
        uwriteln!(f, "  regulator_cfg         = x{}", self.regulator_cfg.hex()).ok();
        uwriteln!(f, "  atqa             = x{}", self.atqa.hex()).ok();
        uwriteln!(f, "  sak1,sak2        = x{} {}", hex!(self.sak1), hex!(self.sak2)).ok();
        uwriteln!(f, "  tl t0 ta tb tc   = x{} {} {} {} {}",
            hex!(self.tl), hex!(self.t0), hex!(self.ta), hex!(self.tb), hex!(self.tc)
        ).ok();
        uwriteln!(f, "  nfc_cfg          = x{}", self.nfc_cfg.hex()).ok();
        uwriteln!(f, "  i2c_addr         = x{}", self.i2c_addr.hex()).ok();
        uwriteln!(f, "  rblock ack,nack  = x{} {}", hex!(self.rblock_ack), hex!(self.rblock_nack))
    }
}

impl ufmt::uDisplay for RegisterBlock {
    fn fmt<W: ?Sized>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite
    {
        use ufmt::uwriteln;
        uwriteln!(f, "").ok();
        uwriteln!(f, "  fifo_count   = x{}", self.fifo_count.hex()).ok();
        uwriteln!(f, "  rf_status    = x{}", self.rf_status.hex()).ok();
        uwriteln!(f, "  rf_txen      = x{}", self.rf_txen.hex()).ok();
        uwriteln!(f, "  rf_baud      = x{}", self.rf_baud.hex()).ok();
        uwriteln!(f, "  rf_rats      = x{}", self.rf_rats.hex()).ok();
        uwriteln!(f, "  main_irq     = x{}", self.main_irq.hex()).ok();
        uwriteln!(f, "  fifo_irq     = x{}", self.fifo_irq.hex()).ok();
        uwriteln!(f, "  aux_irq      = x{}", self.aux_irq.hex()).ok();
        uwriteln!(f, "  main_irq_mask= x{}", self.main_irq_mask.hex()).ok();
        uwriteln!(f, "  fifo_irq_mask= x{}", self.fifo_irq_mask.hex()).ok();
        uwriteln!(f, "  aux_irq_mask = x{}", self.aux_irq_mask.hex()).ok();
        uwriteln!(f, "  nfc_cfg      = x{}", self.nfc_cfg.hex()).ok();
        uwriteln!(f, "  regulator_cfg     = x{}", self.regulator_cfg.hex())
    }
}

impl ufmt::uDisplay for InterruptState {
    fn fmt<W: ?Sized>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite
    {
        use ufmt::uwriteln;

        if self.main != 0 {
            uwriteln!(f,"MAIN:").ok();
            let main = Interrupt::from_bits_truncate(self.main);
            if main.contains(Interrupt::AUX) { uwriteln!(f,"  aux_flag").ok(); }
            if main.contains(Interrupt::FIFO) { uwriteln!(f,"  fifo_flag").ok(); }
            if main.contains(Interrupt::ARBITRATION) { uwriteln!(f,"  arbit_flag").ok(); }
            if main.contains(Interrupt::TX_DONE) { uwriteln!(f,"  tx_done").ok(); }
            if main.contains(Interrupt::RX_DONE) { uwriteln!(f,"  rx_done").ok(); }
            if  self.count > 0             { uwriteln!(f,"  c:{}", self.count).ok(); }
            if main.contains(Interrupt::RX_START) { uwriteln!(f,"  rx_start").ok(); }
            if main.contains(Interrupt::ACTIVE) { uwriteln!(f,"  active").ok(); }
            if main.contains(Interrupt::RF_POWER) { uwriteln!(f,"  rf_pwon").ok(); }
        }

        if self.fifo != 0 {
            uwriteln!(f,"FIFO:").ok();
            let fifo = FifoInterrupt::from_bits_truncate(self.fifo);
            if fifo.contains(FifoInterrupt::EMPTY) { uwriteln!(f,"  fifo_empty").ok(); }
            if fifo.contains(FifoInterrupt::FULL) { uwriteln!(f,"  fifo_full").ok(); }
            if fifo.contains(FifoInterrupt::OVERFLOW) { uwriteln!(f,"  fifo_ovflow").ok(); }
            if fifo.contains(FifoInterrupt::WATER_LEVEL) { uwriteln!(f,"  fifo_wl").ok(); }
        }

        if self.aux != 0 {
            uwriteln!(f,"AUX:").ok();
            let aux = AuxInterrupt::from_bits_truncate(self.aux);
            if aux.contains(AuxInterrupt::FRAMING_ERROR) { uwriteln!(f,"  framing_error").ok(); }
            if aux.contains(AuxInterrupt::CRC_ERROR) { uwriteln!(f,"  crc_error").ok(); }
            if aux.contains(AuxInterrupt::PARITY_ERROR) { uwriteln!(f,"  parity_error").ok(); }
            if aux.contains(AuxInterrupt::FORBIDDEN_ADDRESS_ERROR) { uwriteln!(f,"  ee_prog_err").ok(); }
            if aux.contains(AuxInterrupt::DONE) { uwriteln!(f,"  ee_prog_done").ok(); }
        }

        Ok(())
    }
}

/// Various internal methods to read/write registers and EEPROM data,
/// exposed for convenience but not needed by typical users of this library.
impl<SPI, CS, INT, ANY> Fm11Nc08S <SPI, CS, INT, ANY>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
    ANY: ConfigurationState,
{
    /// Read value of one of the chip's registers.
    pub fn read_reg(&mut self, register: Register) -> u8 {
        self.transact(|spi| {
            query(spi, &[FM11_CMD!(Mode::Read, register), 0])
        }).ok().unwrap()
    }

    /// Write value to one of the chip's registers.
    ///
    /// As not all values may be valid to write, this function is marked as unsafe.
    pub unsafe fn write_reg_unsafe(&mut self, register: Register, value: u8) {
        self.write_reg(register, value);
    }

    /// write value to one of the chip's registers. This is exposed to the public API
    /// for convenience, but only behind an unsafe marker tag.
    fn write_reg(&mut self, register: Register, value: u8) {
        self.transact(|spi| {
            send(spi, &[
                 FM11_CMD!(Mode::Write, register),
                 value
            ])
        }).ok();
    }

    fn unlock_eeprom(&mut self) {
        self.transact(|spi| {
            // write EEPROM magic enable sequence
            send(spi, &[0b11001110, 0b01010101])
        }).ok();
    }

    fn finish_eeprom_write<T>(&mut self, timer: &mut T)
    where
        T: CountDown,
        T::Time: Copy + From<core::time::Duration>,
    {
        let eeprom_delay = EEPROM_WRITE_DELAY;
        // Need to give ~10ms of inactivity for EEPROM block to write
        timer.start(eeprom_delay);
        block!(timer.wait()).ok();

        let aux_irq = AuxInterrupt::from_bits_truncate(self.read_reg(Register::AuxIrq));

        if aux_irq.contains(AuxInterrupt::FORBIDDEN_ADDRESS_ERROR) {
            panic!("Wrote to forbidden EEPROM location");
        }
        if !aux_irq.contains(AuxInterrupt::DONE) {
            panic!("EEPROM did not write");
        }

        self.write_reg(Register::AuxIrq, 0);
    }

    /// Configure the eeprom in FM11 chip.  Should only need to do this once per device.
    fn configure_eeprom<T>(&mut self, config: Configuration, timer: &mut T)
    where
        T: CountDown,
        T::Time: Copy + From<core::time::Duration>,
    {

        // Clear all aux interrupts
        self.write_reg(Register::AuxIrq, 0);

        self.write_eeprom(0xE4, 1, timer, |spi| {
            send(spi, &[config.regu, config.regu])
        }).ok();

        self.write_eeprom(0xE8, 0, timer, |spi| {
            send(spi, &config.ataq.to_be_bytes())?;
            send(spi, &[config.sak1, config.sak2])
        }).ok();

        self.write_eeprom(0xEC, 0, timer, |spi| {
            send(spi, &[
                 config.tl,
                 config.t0,
                 config.nfc,
                 // use I2C addr as magic marker
                 0xA8
            ])?;

            send(spi, &[config.ta, config.tb, config.tc])
        }).ok();

    }

    /// Read out a section of the EEPROM, the buffer is filled completely.
    /// Panics if invalid parameters are passed.
    ///
    /// This method is exposed only for convenience, typical use of this driver will not need it.
    ///
    /// This EEPROM is organized in 256 pages consisting of 4 bytes each,
    /// see the data sheet for more information. Of interest are:
    /// - 3 pages for UID data
    /// - 4 pages for energy harvesting configuration
    /// - 4 pages for NFC configuration data
    pub fn read_eeprom(&mut self, page: u8, offset: u8, buffer: &mut [u8]) {
        assert!(offset < 4);
        let addr = ((page as u16) << 2) + offset as u16;
        assert!(addr as usize + buffer.len() < 1024);
        let cmd = FM11_CMD!(Mode::ReadEeprom, addr);
        // huh?
        let addr = (addr & 0xff) as u8;

        self.transact(|spi| {
            send(spi, &[cmd, addr])?;

            for entry in buffer.iter_mut() {
                block!(spi.send(0))?;
                *entry = block!(spi.read())?;
            }
            Ok(())
        }).ok();
    }

    /// Write data to the EEPROM.
    #[inline]
    pub fn write_eeprom_unsafe<TIMER>(&mut self,
        page: u8, offset: u8,
        timer: &mut TIMER,

        data: &[u8],
    ) -> core::result::Result<(), SPI::Error>
    where
        TIMER: CountDown,
        TIMER::Time: Copy + From<core::time::Duration>,
    {
        self.write_eeprom(page, offset, timer, |spi| {
            send(spi, data)
        })
    }

    #[inline]
    fn write_eeprom<T, TIMER>(&mut self,
        page: u8, offset: u8,
        timer: &mut TIMER,

        f: impl FnOnce(&mut SPI) -> core::result::Result<T, SPI::Error>

    ) -> core::result::Result<T, SPI::Error>
    where
        TIMER: CountDown,
        TIMER::Time: Copy + From<core::time::Duration>,
    {
        assert!(offset < 4);

        self.unlock_eeprom();

        let result = self.transact(|spi| {
            let addr = ((page as u16) << 2) + offset as u16;
            let cmd: u8 = FM11_CMD!(Mode::WriteEeprom, addr);

            send(spi, &[cmd, addr as _])?;

            f(spi)
        });

        self.finish_eeprom_write(timer);
        result
    }


    // pub fn has_interrupt(&mut self) -> nb::Result<(), nfc_chip::Error> {
    //     if self.int.is_low().ok().unwrap() {
    //         Ok(())
    //     } else {
    //         Err(nb::Error::WouldBlock)
    //     }
    // }

    /// Write data to NFC FIFO as fast as possible.
    fn write_fifo(&mut self, buf: &[u8]){

        if buf.is_empty() {
            return;
        }

        self.transact(|spi| {
            // TODO: can the two reads at the end be moved up?
            // Then we could reuse the `send` function
            let cmd = FM11_CMD!(Mode::WriteFifo, 0);
            block!(spi.send(cmd))?;

            // Put extra byte in to ensure spi RX fifo operates continuously.
            // (assumes count >= 1)
            block!(spi.send(buf[0]))?;

            for byte in buf {
                block!(spi.send(*byte))?;
                block!(spi.read())?;
            }

            // for header + that extra byte.
            block!(spi.read())?;
            block!(spi.read())?;
            Ok(())

        }).ok();
    }

    fn is_transmitting(&mut self) -> bool {
        let status = TransceptionStatus::from_bits_truncate(self.read_reg(Register::RfStatus));
        status.contains(TransceptionStatus::TRANSMITTING)
    }

    /// Read data from NFC FIFO as fast as possible.
    fn read_fifo(&mut self, count: usize){
        assert!(count > 0);
        assert!(self.offset + count < self.packet.len());

        let buf: &mut [u8] = &mut self.packet[self.offset..];

        // can't use `self.transact` here due to unique referfence to self.packet
        self.cs.set_low().ok();

        block!(self.spi.send(FM11_CMD!(Mode::ReadFifo, 0))).ok();
        // Put extra byte in to ensure self.spi RX fifo operates continuously.
        // (assumes count >= 1)
        block!(self.spi.send(0)).ok();

        // Skip first byte
        block!(self.spi.read()).ok().unwrap();

        for entry in buf.iter_mut().take(count - 1) {
            block!(self.spi.send(0)).ok();
            *entry = block!(self.spi.read()).ok().unwrap();
        }

        // for that extra byte.
        buf[count - 1] = block!(self.spi.read()).ok().unwrap();

        self.cs.set_high().ok();
    }

    fn main_irq(&mut self) -> Interrupt {
        Interrupt::from_bits_truncate(self.read_reg(Register::MainIrq))
    }
    pub fn dump_configuration(&mut self) -> FullConfiguration {
        // cf. Fig 2-1 EEPROM space assignment in data sheet (rev 1.1)
        let mut arr = [0u8; 12];

        // ISO 14443-A UID
        let uid = self.uid();

        // energy harvesting configuration
        self.read_eeprom(0xE4, 1, &mut arr[..1]);
        let regulator_cfg = arr[0];

        // ISO 14443A-3 configuration
        self.read_eeprom(0xE8, 0, &mut arr[..4]);
        let [atqa_hi, atqa_lo, sak1, sak2, ..] = arr;
        let atqa = u16::from_be_bytes([atqa_hi, atqa_lo]);

        // NFC configuration
        self.read_eeprom(0xEC, 0, &mut arr);
        let [
            tl, t0, nfc_cfg, i2c_addr,
            ta, tb, tc, _,
            _, _, rblock_ack, rblock_nack, ..] = arr;

        FullConfiguration {
            uid,
            regulator_cfg,
            atqa, sak1, sak2,
            tl, t0, ta, tb, tc, i2c_addr, nfc_cfg, rblock_ack, rblock_nack,
        }
    }

    pub fn dump_registers(&mut self) -> RegisterBlock {
        use Register::*;
        RegisterBlock {
            fifo_count: self.read_reg(FifoCount),
            rf_status: self.read_reg(RfStatus),
            rf_txen: self.read_reg(RfTxEn),
            rf_baud: self.read_reg(RfBaud),
            rf_rats: self.read_reg(RfRats),
            main_irq: self.read_reg(MainIrq),
            fifo_irq: self.read_reg(FifoIrq),
            aux_irq: self.read_reg(AuxIrq),
            main_irq_mask: self.read_reg(MainIrqMask),
            fifo_irq_mask: self.read_reg(FifoIrqMask),
            aux_irq_mask: self.read_reg(AuxIrqMask),
            nfc_cfg: self.read_reg(NfcConfig),
            regulator_cfg: self.read_reg(RegulatorConfig),
        }
    }

    pub fn dump_interrupts(&mut self) -> InterruptState {
        let interrupts = InterruptState {
            main: self.read_reg(Register::MainIrq),
            fifo: self.read_reg(Register::FifoIrq),
            aux: self.read_reg(Register::AuxIrq),
            count: self.read_reg(Register::FifoCount),
        };

        self.write_reg(Register::MainIrq, 0);
        self.write_reg(Register::FifoIrq, 0);
        self.write_reg(Register::AuxIrq, 0);

        interrupts
    }

    /// 7 byte ISO 14443-A UID, used during collision detection and set by manufacturer
    pub fn uid(&mut self) -> [u8; 7] {
        let mut buf = [0u8; 9];
        self.read_eeprom(0x00, 0, &mut buf);
        let [sn0, sn1, sn2, _bcc0, sn3, sn4, sn5, sn6, _bcc1] = buf;
        let uid = [sn0, sn1, sn2, sn3, sn4, sn5, sn6];
        uid
    }

}

