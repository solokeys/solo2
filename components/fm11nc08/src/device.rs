use core::marker::PhantomData;

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
use crate::traits::nfc;

pub enum Mode {
    Write = 0b000,
    Read = 0b001,
    WriteEeprom = 0b010,
    ReadEeprom = 0b011,
    WriteFifo = 0b100,
    ReadFifo = 0b101,
}

// bitflags::bitflags! {
//     pub struct Mode: u8 {
//         const WRITE = 0b000;
//         const READ = 0b001;
//         const WRITE_EEPROM = 0b010;
//         const READ_EEPROM = 0b011;
//         const WRITE_FIFO = 0b100;
//         const READ_FIFO = 0b101;
//     }
// }

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Register {
    #[doc(hidden)]
    FifoAccess = 0,
    /// Write this register to flush FIFO
    FifoFlush = 1,
    /// Byte count left in FIFO
    FifoCount = 2,
    /// Working status of  RF interface
    RfStatus = 3,
    /// RF response enable
    RfTxEn = 4,
    #[doc(hidden)]
    /// RF transmission baud selection
    RfBaud = 5,
    /// RATS content received by RF interface
    RfRats = 6,
    /// Main interrupt flag
    MainIrq = 7,
    /// FIFO interrupt flag
    FifoIrq = 8,
    /// Auxiliary interrupt flag
    AuxIrq = 9,
    #[doc(hidden)]
    MainIrqMask = 10,
    #[doc(hidden)]
    FifoIrqMask = 11,
    #[doc(hidden)]
    AuxIrqMask = 12,
    /// NFC configuration
    NfcConfig = 13,
    /// Regulator configuration
    RegulatorConfig = 14,
}

impl Register {
    pub fn spi_address(&self) -> u8 {
        *self as u8
    }
}

// bitflags::bitflags! {
//     pub struct Register: u8 {
//         const FIFO_ACCESS = 1 << 0;
//         const FIFO_FLUSH = 1 << 1;
//         const FIFO_COUNT = 1 << 2;
//         const RF_STATUS = 1 << 3;
//         const RF_TX_EN = 1 << 4;
//         const RF_BAUD = 1 << 5;
//         const RF_RATS = 1 << 6;
//         const MAIN_IRQ = 1 << 7;
//         const FIFO_IRQ = 1 << 8;
//         const AUXI_RQ = 1 << 9;
//         const MAIN_IRQ_MASK = 1 << 10;
//         const FIFO_IRQ_MASK = 1 << 11;
//         const AUX_IRQ_MASK = 1 << 12;
//         const NFC_CFG = 1 << 13;
//         const REGU_CFG = 1 << 14;
//     }
// }

bitflags::bitflags! {
    pub struct Interrupt: u8 {
        const AUX = 1 << 0;
        const FIFO = 1 << 1;
        const ARBITRATION = 1 << 2;
        const TX_DONE = 1 << 3;
        const RX_DONE = 1 << 4;
        const RX_START = 1 << 5;
        const ACTIVE = 1 << 6;
        const RF_POWER = 1 << 7;
    }
}

bitflags::bitflags! {
    pub struct FifoInterrupt: u8 {
        const EMPTY = 1 << 0;
        const FULL = 1 << 1;
        const OVERFLOW = 1 << 2;
        const WATER_LEVEL = 1 << 3;
    }
}

bitflags::bitflags! {
    pub struct AuxInterrupt: u8 {
        const FRAMING_ERROR = 1 << 3;
        const CRC_ERROR = 1 << 4;
        const PARITY_ERROR = 1 << 5;
        const FORBIDDEN_ADDRESS_ERROR = 1 << 6;
        const DONE = 1 << 7;
    }
}

bitflags::bitflags! {
    pub struct TransceptionStatus: u8 {
        const TRANSMITTING = 1 << 0;
        const RECEIVING = 1 << 1;
    }
}


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

pub struct Configuration {
    pub regu: u8,
    pub ataq: u16,
    pub sak1: u8,
    pub sak2: u8,
    pub tl: u8,
    pub t0: u8,
    pub ta: u8,
    pub tb: u8,
    pub tc: u8,
    pub nfc: u8,
}

pub trait ChipState {}

pub struct Undefined;
impl ChipState for Undefined {}
pub struct Configured;
impl ChipState for Configured {}

pub struct FM11NC08 <SPI, CS, INT, STATE>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
    STATE: ChipState,
{
    spi: SPI,
    cs: CS,
    pub int: INT,
    __: PhantomData<STATE>,
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


impl<SPI, CS, INT> FM11NC08<SPI, CS, INT, Undefined>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
{
    // The embedded-hal Timer trait has no trait bounds on its Time type,
    // not even Copy. So to remove the dependency of this crate on e.g. lpc55-hal,
    // where there's a sane choice of time, we have consumers pass in the value
    // for "10 milliseconds"; hopefully this can be improved later (e.g. by requiring
    // Time to be an explicit embedded-time type)
    pub fn configure<T>(
        mut self,
        config: Configuration,
        force_reconfiguration: bool,
        timer: &mut T,
        ten_milliseconds_value: T::Time,
    )
    ->
        Option<FM11NC08<SPI, CS, INT, Configured>>
    where
        T: CountDown,
        T::Time: Copy,
    {
        let current_regu_config = self.read_reg(Register::RegulatorConfig);

        if current_regu_config == 0xff {
            // No nfc chip connected
            return None;
        }

        let reconfig = (current_regu_config != config.regu) || force_reconfiguration;
        if reconfig {
            info!("{}", self.dump_eeprom() ).ok();
            info!("{}", self.dump_registers() ).ok();

            info!("writing EEPROM").ok();

            self.configure_eeprom(config, timer, ten_milliseconds_value);
        } else {
            info!("EEPROM already initialized.").ok();
        }

        self.configure_interrupts();

        Some(FM11NC08 {
            spi: self.spi,
            cs: self.cs,
            int: self.int,
            __: PhantomData,
            packet: self.packet,
            offset: self.offset,
            current_frame_size: self.current_frame_size
        })
    }

    pub fn new(spi: SPI, cs: CS, int: INT) -> Self {
        FM11NC08 {
            spi,
            cs,
            int,
            __: PhantomData,
            packet: [0u8; 256],
            offset: 0usize,
            current_frame_size: 128,
        }
    }

}

fn send<SPI: FullDuplex<u8>>(spi: &mut SPI, data: &[u8]) -> core::result::Result<(), SPI::Error> {
    for byte in data {
        block!(spi.send(*byte))?;
    }
    for _ in 0..data.len() {
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

impl<SPI, CS, INT, STATE> FM11NC08 <SPI, CS, INT, STATE>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
    STATE: ChipState,
{
    pub fn configure_interrupts(&mut self) {
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
        // let regu_powered = (0b11 << 4) | (0b10 << 2) | (0b11 << 0);
        // fm.write_reg(Register::RegulatorConfig, regu_powered);
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

    pub fn write_reg(&mut self, register: Register, data: u8) {
        self.transact(|spi| {
            send(spi, &[
                 FM11_CMD!(Mode::Write, register),
                 data
            ])
        }).ok();
    }

    pub fn read_reg(&mut self, addr: Register) -> u8 {
        self.transact(|spi| {
            query(spi, &[FM11_CMD!(Mode::Read, addr), 0])
        }).ok().unwrap()
    }

    pub fn read_reg_raw(&mut self, addr: u8) -> u8 {
        self.transact(|spi| {
            query(spi, &[FM11_CMD!(Mode::Read, addr), 0])
        }).ok().unwrap()
    }

    fn unlock_eeprom(&mut self) {
        self.transact(|spi| {
            // write EEPROM magic enable sequence
            send(spi, &[0b11001110, 0b01010101])
        }).ok();
    }

    fn finish_eeprom_write<T>(&mut self, timer: &mut T, eeprom_delay: T::Time)
    where
        T: CountDown,
        T::Time: Copy,
    {
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

    #[inline]
    fn write_eeprom<T, TIMER>(&mut self,
        page: u8, offset: u8,
        timer: &mut TIMER,
        eeprom_delay: TIMER::Time,

        f: impl FnOnce(&mut SPI) -> core::result::Result<T, SPI::Error>

    ) -> core::result::Result<T, SPI::Error>
    where
        TIMER: CountDown,
        TIMER::Time: Copy,
    {
        assert!(offset < 4);

        self.unlock_eeprom();

        let result = self.transact(|spi| {
            let addr = (page << 2) + offset;
            let cmd: u8 = FM11_CMD!(Mode::WriteEeprom, addr);

            send(spi, &[cmd, addr as _])?;

            f(spi)
        });

        self.finish_eeprom_write(timer, eeprom_delay);
        result
    }

    /// Configure the eeprom in FM11 chip.  Should only need to do this once per device.
    pub fn configure_eeprom<T>(&mut self, config: Configuration, timer: &mut T, ten_milliseconds_value: T::Time)
    where
        T: CountDown,
        T::Time: Copy,
    {

        // Clear all aux interrupts
        self.write_reg(Register::AuxIrq, 0);

        self.write_eeprom(0xE4, 1, timer, ten_milliseconds_value, |spi| {
            send(spi, &[config.regu, config.regu])
        }).ok();

        self.write_eeprom(0xE8, 0, timer, ten_milliseconds_value, |spi| {
            send(spi, &config.ataq.to_be_bytes())?;
            send(spi, &[config.sak1, config.sak2])
        }).ok();

        self.write_eeprom(0xEC, 0, timer, ten_milliseconds_value, |spi| {
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

    pub fn read_eeprom(&mut self, addr: u16, array: &mut [u8]) {
        assert!(array.len() <= 16);

        let cmd = FM11_CMD!(Mode::ReadEeprom, addr);
        let addr = (addr & 0xff) as u8;

        self.transact(|spi| {
            send(spi, &[cmd, addr])?;

            for entry in array.iter_mut() {
                block!(spi.send(0))?;
                *entry = block!(spi.read())?;
            }
            Ok(())
        }).ok();
    }

    pub fn enabled(self,) -> Self {
        self
    }

    // pub fn has_interrupt(&mut self) -> nb::Result<(), nfc::Error> {
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

    pub fn main_irq(&mut self) -> Interrupt {
        Interrupt::from_bits_truncate(self.read_reg(Register::MainIrq))
    }

    // pub fn fifo_irq(&mut self) -> FifoInterrupt {
    //     FifoInterrupt::from_bits_truncate(self.read_reg(Register::FifoIrq));
    // }

    pub fn read_packet(&mut self, buf: &mut [u8]) -> Result<nfc::State, nfc::Error>{

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
                    return Ok(nfc::State::NewSession(l as u8));
                } else {
                    return Ok(nfc::State::Continue(l as u8));
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
            Err(nfc::Error::NewSession)
        } else {
            Err(nfc::Error::NoActivity)
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

    pub fn send_packet(&mut self, buf: &[u8]) -> Result<(), nfc::Error>{

        // Write in chunks of 24
        for chunk in buf.chunks(24) {
            self.write_fifo(chunk);
            self.wait_for_transmission().map_err(|_| nfc::Error::NoActivity)?;
        }

        Ok(())

    }

    pub fn release(self) -> (SPI, CS, INT) {
        (self.spi, self.cs, self.int)
    }

}

impl<SPI, CS, INT, STATE> nfc::Device for FM11NC08 <SPI, CS, INT, STATE>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
    STATE: ChipState,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<nfc::State, nfc::Error>{
        self.read_packet(buf)
    }

    fn send(&mut self,buf: &[u8]) -> Result<(), nfc::Error>{
        self.send_packet(buf)
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


/// For logging
pub struct Eeprom {
    regu_cfg: u8,
    atqa: u16,
    sak1: u8,
    sak2: u8,
    tl: u8,
    t0: u8,
    ta: u8,
    tb: u8,
    tc: u8,
    nfc_cfg: u8,
    i2c_addr: u8,
    rblock_ack: u8,
    rblock_nack: u8,
}

pub struct InterruptState {
    main: u8,
    fifo: u8,
    aux: u8,
    count: u8,
}

pub struct RegisterBlock {
    fifo_count: u8,
    rf_status: u8,
    rf_txen: u8,
    rf_baud: u8,
    rf_rats: u8,
    main_irq: u8,
    fifo_irq: u8,
    aux_irq: u8,
    main_irq_mask: u8,
    fifo_irq_mask: u8,
    aux_irq_mask: u8,
    nfc_cfg: u8,
    regu_cfg: u8,
}



impl ufmt::uDisplay for Eeprom {
    fn fmt<W: ?Sized>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite
    {
        use ufmt::uwriteln;
        uwriteln!(f, "").ok();
        uwriteln!(f, "  regu_cfg         = x{}", self.regu_cfg.hex()).ok();
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
        uwriteln!(f, "  regu_cfg     = x{}", self.regu_cfg.hex())
    }
}




impl ufmt::uDisplay for InterruptState {
    fn fmt<W: ?Sized>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite
    {
        use ufmt::uwriteln;

        // PUT BACK IN!

        // if self.main != 0 {
        //     // let count =
        //     //     if (main & (1 << 4)) != 0 || (main & (1<<5)) != 0 {
        //     //         fm.read_reg(Register::FifoCount)
        //     //     } else { 0 };

        //     uwriteln!(f,"MAIN:").ok();
        //     if (self.main & (Interrupt::Aux as u8)) != 0 { uwriteln!(f,"  aux_flag").ok(); }
        //     if (self.main & (Interrupt::Fifo as u8)) != 0 { uwriteln!(f,"  fifo_flag").ok(); }
        //     if (self.main & (Interrupt::Arbitration as u8)) != 0 { uwriteln!(f,"  arbit_flag").ok(); }
        //     if (self.main & (Interrupt::TxDone as u8)) != 0 { uwriteln!(f,"  tx_done").ok(); }
        //     if (self.main & (Interrupt::RxDone as u8)) != 0 { uwriteln!(f,"  rx_done").ok(); }
        //     if  self.count > 0             { uwriteln!(f,"  c:{}", self.count).ok(); }
        //     if (self.main & (Interrupt::RxStart as u8)) != 0 { uwriteln!(f,"  rx_start").ok(); }
        //     if (self.main & (Interrupt::Active as u8)) != 0 { uwriteln!(f,"  active").ok(); }
        //     if (self.main & (Interrupt::RfPower as u8)) != 0 { uwriteln!(f,"  rf_pwon").ok(); }
        // }

        // uwriteln!(f, "{:?}",
        // if self.fifo != 0 {
        //     uwriteln!(f,"FIFO:").ok();
        //     if (self.fifo & (1 << 0)) != 0 { uwriteln!(f,"  fifo_empty").ok(); }
        //     if (self.fifo & (1 << 1)) != 0 { uwriteln!(f,"  fifo_full").ok(); }
        //     if (self.fifo & (1 << 2)) != 0 { uwriteln!(f,"  fifo_ovflow").ok(); }
        //     if (self.fifo & (1 << 3)) != 0 { uwriteln!(f,"  fifo_wl").ok(); }
        // }

        // if self.aux != 0 {
        //     uwriteln!(f,"AUX:").ok();
        //     if (self.aux & (1 << 3)) != 0 { uwriteln!(f,"  framing_error").ok(); }
        //     if (self.aux & (1 << 4)) != 0 { uwriteln!(f,"  crc_error").ok(); }
        //     if (self.aux & (1 << 5)) != 0 { uwriteln!(f,"  parity_error").ok(); }
        //     if (self.aux & (1 << 6)) != 0 { uwriteln!(f,"  ee_prog_err").ok(); }
        //     if (self.aux & (1 << 7)) != 0 { uwriteln!(f,"  ee_prog_done").ok(); }
        // }

        Ok(())
    }
}

impl<SPI, CS, INT, STATE> FM11NC08 <SPI, CS, INT, STATE>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
    STATE: ChipState,
{
    pub fn dump_registers(&mut self) -> RegisterBlock {

        let mut regs = [0u8; 15];

        for i in 2 .. 15 {
            regs[i] = self.read_reg_raw(i as u8);
        }

        RegisterBlock {
            fifo_count: regs[2],
            rf_status: regs[3],
            rf_txen: regs[4],
            rf_baud: regs[5],
            rf_rats: regs[6],
            main_irq: regs[7],
            fifo_irq: regs[8],
            aux_irq: regs[9],
            main_irq_mask: regs[10],
            fifo_irq_mask: regs[11],
            aux_irq_mask: regs[12],
            nfc_cfg: regs[13],
            regu_cfg: regs[14],
        }
    }

    pub fn dump_interrupts(&mut self) -> InterruptState {
        let main = self.read_reg(Register::MainIrq);
        let fifo = self.read_reg(Register::FifoIrq);
        let aux = self.read_reg(Register::AuxIrq);
        let count = self.read_reg(Register::FifoCount);

        self.write_reg(Register::MainIrq, 0);
        self.write_reg(Register::FifoIrq, 0);
        self.write_reg(Register::AuxIrq, 0);

        InterruptState { main, fifo, aux, count }
    }


    pub fn dump_eeprom(&mut self) -> Eeprom {
        let mut arr = [0u8; 16];
        let mut double_byte = [0u8 ; 2];
        self.read_eeprom(0x390, &mut arr);

        let regu_cfg = arr[1];

        self.read_eeprom(0x3a0 + 0, &mut arr);

        double_byte.clone_from_slice(&arr[0 .. 2]);
        let atqa = u16::from_be_bytes(double_byte);
        let sak1 = arr[2];
        let sak2 = arr[3];

        self.read_eeprom(0x3b0 + 0, &mut arr);
        let tl = arr[0];
        let t0 = arr[1];
        let nfc_cfg = arr[2];
        let i2c_addr = arr[3];

        let ta = arr[4];
        let tb = arr[5];
        let tc = arr[6];
        let rblock_ack = arr[10];
        let rblock_nack = arr[11];

        Eeprom {
            regu_cfg,
            atqa,
            sak1,
            sak2,
            tl,
            t0,
            ta,
            tb,
            tc,
            i2c_addr,
            nfc_cfg,
            rblock_ack,
            rblock_nack,
        }
    }
}

