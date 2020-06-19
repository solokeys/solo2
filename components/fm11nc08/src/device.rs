use nb::{block};

use lpc55_hal as hal;

use hal::{

    traits::wg::{
        spi::{
            FullDuplex,
        },
        digital::v2::InputPin,
        digital::v2::OutputPin,
        timer::CountDown,
    },
    drivers::{
        Timer,
    },
};

use hal::{
    time::*,
    typestates::{
        init_state,
    },
    peripherals::{
        ctimer::Ctimer,
    }
};
use crate::traits::nfc;
use logging::hex;
use logging::hex::*;

// use cortex_m_semihosting::{dbg, hprint, hprintln, heprintln};
use funnel::{
    info,
};

pub enum Mode {
    Write = 0b000,
    Read = 0b001,
    WriteEeprom = 0b010,
    ReadEeprom = 0b011,
    WriteFifo = 0b100,
    ReadFifo = 0b101,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Register {
    FifoAccess = 0,
    FifoFlush = 1,
    FifoCount = 2,
    RfStatus = 3,
    RfTxEn = 4,
    RfBaud = 5,
    RfRats = 6,
    MainIrq = 7,
    FifoIrq = 8,
    AuxIrq = 9,
    MainIrqMask = 10,
    FifoIrqMask = 11,
    AuxIrqMask = 12,
    NfcCfg = 13,
    ReguCfg = 14,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Interrupt {
    Aux = (1 << 0),
    Fifo = (1 << 1),
    Arbitration = (1 << 2),
    TxDone = (1 << 3),
    RxDone = (1 << 4),
    RxStart = (1 << 5),
    Active = (1 << 6),
    RfPower = (1 << 7),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FifoInterrupt {
    Empty = (1 << 0),
    Full = (1 << 1),
    OverFlow = (1 << 2),
    WaterLevel = (1 << 3),
}



macro_rules! FM11_CMD {
    ($mode:expr, $addr:expr) => {
        match $mode {
            Mode::WriteEeprom | Mode::ReadEeprom=> {
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

pub struct FM11NC08 <SPI, CS, INT>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
{
    spi: SPI,
    cs: CS,
    pub int: INT,
    packet: [u8; 256],
    offset: usize,
}


impl<SPI, CS, INT> FM11NC08 <SPI, CS, INT>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
{
    pub fn new(spi: SPI, cs: CS, int: INT) -> Self {
        Self {
            spi: spi,
            cs: cs,
            int: int,
            packet: [0u8; 256],
            offset: 0usize,
        }
    }

    pub fn write_reg(&mut self, addr: Register, data: u8) {
        self.cs.set_low().ok();

        block!( self.spi.send(FM11_CMD!(Mode::Write, addr)) ).ok();
        block!( self.spi.send(data) ).ok();

        block!( self.spi.read() ).ok();
        block!( self.spi.read() ).ok();

        self.cs.set_high().ok();
    }

    pub fn read_reg(&mut self, addr: Register) -> u8 {
        self.cs.set_low().ok();

        block!( self.spi.send(FM11_CMD!(Mode::Read, addr)) ).ok();
        block!( self.spi.send(0) ).ok();

        block!( self.spi.read() ).ok();
        let data = block!( self.spi.read() ).ok().unwrap();

        self.cs.set_high().ok();

        data
    }

    pub fn read_reg_raw(&mut self, addr: u8) -> u8 {
        self.cs.set_low().ok();

        block!( self.spi.send(FM11_CMD!(Mode::Read, addr)) ).ok();
        block!( self.spi.send(0) ).ok();

        block!( self.spi.read() ).ok();
        let data = block!( self.spi.read() ).ok().unwrap();

        self.cs.set_high().ok();

        data
    }



    fn start_write(&mut self, addr: u16){

        let cmd : u8  = FM11_CMD!(Mode::WriteEeprom, addr);

        self.cs.set_low().ok();

        // Write EEPROM magic enable sequence
        block!( self.spi.send( 0b11001110u8 )).ok();
        block!( self.spi.send( 0b01010101u8 )).ok();

        for _ in 0 .. 2 { block!( self.spi.read(  )).ok().unwrap(); }

        self.cs.set_high().ok();
        self.cs.set_low().ok();

        block!( self.spi.send( cmd )).ok();
        block!( self.spi.send( addr as u8)).ok();

        for _ in 0 .. 2 { block!( self.spi.read(  )).ok().unwrap(); }
    }

    fn end_write(&mut self, timer: &mut Timer<impl Ctimer<init_state::Enabled>>){
        self.cs.set_high().ok();

        // Need to give ~10ms of unactivity for eeprom block to write
        timer.start(10.ms()); block!(timer.wait()).ok();

        let aux_irq = self.read_reg(Register::AuxIrq);
        if (aux_irq & (1 << 6)) != 0 {
            panic!("Wrote to forbidden EEPROM location");
        }
        if (aux_irq & (1 << 7)) == 0 {
            panic!("EEPROM did not write");
        }

        self.write_reg(Register::AuxIrq, 0);
    }

    /// Configure the eeprom in FM11 chip.  Should only need to do this once per device.
    pub fn configure(&mut self, config: Configuration, timer: &mut Timer<impl Ctimer<init_state::Enabled>>){

        // Clear all aux interrupts
        self.write_reg(Register::AuxIrq, 0);

        self.start_write(0x390 + 1);

        block!( self.spi.send( config.regu )).ok();
        block!( self.spi.send( config.regu )).ok();
        for _ in 0 .. 2 { block!( self.spi.read(  )).ok().unwrap(); }

        self.end_write(timer);

        self.start_write(0x3A0);

        block!( self.spi.send( config.ataq.to_be_bytes()[0] )).ok();
        block!( self.spi.send( config.ataq.to_be_bytes()[1] )).ok();
        block!( self.spi.send( config.sak1)).ok();
        block!( self.spi.send( config.sak2)).ok();

        for _ in 0 .. 4 { block!( self.spi.read(  )).ok().unwrap(); }

        self.end_write(timer);

        self.start_write(0x3b0);

        block!( self.spi.send( config.tl )).ok();
        block!( self.spi.send( config.t0 )).ok();
        block!( self.spi.send( config.nfc )).ok();
        block!( self.spi.send( 0xA8 )).ok();          // use I2C addr as magic marker

        for _ in 0 .. 4 { block!( self.spi.read(  )).ok().unwrap(); }

        block!( self.spi.send( config.ta )).ok();
        block!( self.spi.send( config.tb )).ok();
        block!( self.spi.send( config.tc )).ok();

        for _ in 0 .. 3 { block!( self.spi.read(  )).ok().unwrap(); }

        self.end_write(timer);

    }

    pub fn read_eeprom(&mut self, addr: u16, array: &mut [u8]) {
        assert!(array.len() <= 16);

        let cmd = FM11_CMD!(Mode::ReadEeprom, addr);
        let addr = (addr & 0xff) as u8;
        self.cs.set_low().ok();
        block!( self.spi.send( cmd )).ok();
        block!( self.spi.send( addr )).ok();

        block!( self.spi.read(  )).ok().unwrap();
        block!( self.spi.read(  )).ok().unwrap();

        for i in 0 .. array.len() {
            block!( self.spi.send( 0 )  ).ok();
            array[i] = block!( self.spi.read(  )).ok().unwrap();
        }
        self.cs.set_high().ok();
    }

    pub fn enabled(self,) -> Self {
        self
    }

    pub fn has_interrupt(&mut self, ) -> nb::Result<(), nfc::Error> {
        if self.int.is_low().ok().unwrap() {
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }

    /// Write data to NFC FIFO as fast as possible.
    fn write_fifo(&mut self, buf: &[u8]){
        if buf.len() == 0 {
            return;
        }
        self.cs.set_low().ok();

        block!( self.spi.send(FM11_CMD!(Mode::WriteFifo, 0)) ).ok();

        // Put extra byte in to ensure spi RX fifo operates continuously.
        // (assumes count >= 1)
        block!( self.spi.send(buf[0]) ).ok();

        for i in 1 .. buf.len() {
            block!( self.spi.send(buf[i as usize]) ).ok();
            block!( self.spi.read() ).ok().unwrap();
        }

        // for header + that extra byte.
        block!( self.spi.read() ).ok().unwrap();
        block!( self.spi.read() ).ok().unwrap();

        self.cs.set_high().ok();
    }

    /// Read data from NFC FIFO as fast as possible.
    fn read_fifo(&mut self, /*buf: &mut [u8],*/ count: u8){
        let buf: &mut [u8] = &mut self.packet[self.offset..];
        self.cs.set_low().ok();

        block!( self.spi.send(FM11_CMD!(Mode::ReadFifo, 0)) ).ok();

        // Put extra byte in to ensure spi RX fifo operates continuously.
        // (assumes count >= 1)
        block!( self.spi.send(0) ).ok();

        // Skip first byte
        block!( self.spi.read() ).ok().unwrap();

        for i in 0 .. (count-1) {
            block!( self.spi.send(0) ).ok();
            buf[i as usize] = block!( self.spi.read() ).ok().unwrap();
        }

        // for that extra byte.
        buf[(count-1) as usize] = block!( self.spi.read() ).ok().unwrap();

        self.cs.set_high().ok();
    }

    pub fn read_packet(&mut self, buf: &mut [u8]) -> Result<nfc::State, nfc::Error>{

        let main_irq = self.read_reg(Register::MainIrq);
        let mut new_session = false;

        let do_flush = if main_irq & (Interrupt::TxDone as u8) != 0 {
            // Need to turn off transmit mode
            let count = self.read_reg(Register::FifoCount);
            info!("off transmit (-{}) {}", count, logging::hex!(main_irq)).ok();
            self.write_reg(Register::RfTxEn, 0x00);
            true
        } else {
            false
        };

        let fifo_irq = if (main_irq & Interrupt::Fifo as u8) != 0 {
            self.read_reg(Register::FifoIrq)
        } else {
            0
        };

        let aux_irq = if (main_irq & Interrupt::Aux as u8) != 0 {
            self.read_reg(Register::AuxIrq)
        } else {
            0
        };

        // check for overflow
        if (fifo_irq & (1 << 2)) != 0 {
            info!("!OF! {} @{}", self.read_reg(Register::FifoCount), hal::get_cycle_count()/96_00).ok();
            info!("{} {} {}",
                    logging::hex!(main_irq),
                    logging::hex!(fifo_irq),
                    logging::hex!(aux_irq),
                ).ok();

            // self.write_reg(Register::FifoFlush, 0xff);
        }

        if main_irq & (Interrupt::Active as u8) != 0{
            self.offset = 0;
            new_session = true;
            info!("Active").ok();
        }

        if main_irq & (Interrupt::RxStart as u8) != 0{
            self.offset = 0;
            info!("RxStart").ok();
        }

        if main_irq & (Interrupt::RxDone as u8) != 0 {
            let count = self.read_reg(Register::FifoCount);
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
                if do_flush {
                    self.write_reg(Register::FifoFlush, 0xaa);
                }
                if new_session {
                    return Ok(nfc::State::NewSession(l as u8));
                } else {
                    return Ok(nfc::State::Continue(l as u8));
                }
            }
        }

            /* water level */
        let rf_status = self.read_reg(Register::RfStatus);
        if (fifo_irq & (1 << 3) != 0) && (rf_status & (1 << 0)) == 0 {
            let count = self.read_reg(Register::FifoCount);
            info!("WL {}", count).ok();
            self.read_fifo(count);
            logging::dump_hex(&self.packet[self.offset ..], count as usize);
            self.offset += count as usize;
            if count == 32 {
                info!("warning: potential ovflw").ok();
            }
        }
        if do_flush {
            self.write_reg(Register::FifoFlush, 0xaa);
        }

        info!(". {},{},{}",
            // logging::hex!(rf_status),
            logging::hex!(main_irq),
            logging::hex!(fifo_irq),
            logging::hex!(aux_irq),
        ).ok();

        if new_session {
            Err(nfc::Error::NewSession)
        } else {
            Err(nfc::Error::NoActivity)
        }

    }

    fn wait_for_transmission(&mut self) -> Result<(), ()>{
        let mut i = 0;

        self.write_reg(Register::RfTxEn, 0x55);
        let mut rf_status = self.read_reg(Register::RfStatus);
        while (rf_status & 1) == 0 {
            i += 1;
            if i > 100 {
                info!("Chip is not transmitting.").ok();
                break;
            }
            rf_status = self.read_reg(Register::RfStatus);
        }
        let initial_count = self.read_reg(Register::FifoCount);
        let mut current_count = initial_count;
        if current_count >= 8 {

            let mut fifo_irq = self.read_reg(Register::FifoIrq);
            if (rf_status & 1) == 1 {

                while (fifo_irq & (FifoInterrupt::WaterLevel as u8)) == 0 {
                    i += 1;
                    if i > 300 {
                        info!("TX transmission timeout.").ok();
                        break;
                    }
                    fifo_irq = self.read_reg(Register::FifoIrq);
                }
            }

            current_count = self.read_reg(Register::FifoCount);
            let aux_irq = self.read_reg(Register::AuxIrq);
            let rf_status = self.read_reg(Register::RfStatus);
            info!("tx {}->{}. {} {} {}",
                initial_count,
                current_count,
                logging::hex!(rf_status),
                logging::hex!(aux_irq),
                logging::hex!(fifo_irq),
            ).ok();

            if (fifo_irq & (FifoInterrupt::WaterLevel as u8)) != 0 {
                return Ok(())
            } else {
                return Err(())
            }
        }
        Ok(())
    }

    pub fn send_packet(&mut self, buf: &[u8]) -> Result<(), nfc::Error>{

        // Write in chunks of 24
        for i in 0 .. buf.len()/24 {
            info!("24 chunk").ok();
            self.write_fifo(&buf[i * 24 .. i * 24 + 24]);

            if ! self.wait_for_transmission().is_ok() {
                return Err(nfc::Error::NoActivity);
            }
        }

        // Write remainder
        self.write_fifo(&buf[ (buf.len()/24) * 24 .. buf.len() ]);

        self.wait_for_transmission().ok();

        Ok(())

    }

    pub fn release(self) -> (SPI, CS, INT) {
        (self.spi, self.cs, self.int)
    }

}

impl<SPI, CS, INT> nfc::Device for FM11NC08 <SPI, CS, INT>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<nfc::State, nfc::Error>{
        self.read_packet(buf)
    }

    fn send(&mut self,buf: &[u8]) -> Result<(), nfc::Error>{
        self.send_packet(buf)
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
        uwriteln!(f, "  regu_cfg         = x{}", hex!(self.regu_cfg)).ok();
        uwriteln!(f, "  atqa             = x{}", hex!(self.atqa)).ok();
        uwriteln!(f, "  sak1,sak2        = x{} {}", hex!(self.sak1), hex!(self.sak2)).ok();
        uwriteln!(f, "  tl t0 ta tb tc   = x{} {} {} {} {}",
            hex!(self.tl), hex!(self.t0), hex!(self.ta), hex!(self.tb), hex!(self.tc)
        ).ok();
        uwriteln!(f, "  nfc_cfg          = x{}", hex!(self.nfc_cfg)).ok();
        uwriteln!(f, "  i2c_addr         = x{}", hex!(self.i2c_addr)).ok();
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
        uwriteln!(f, "  fifo_count   = x{}", hex!(self.fifo_count)).ok();
        uwriteln!(f, "  rf_status    = x{}", hex!(self.rf_status)).ok();
        uwriteln!(f, "  rf_txen      = x{}", hex!(self.rf_txen)).ok();
        uwriteln!(f, "  rf_baud      = x{}", hex!(self.rf_baud)).ok();
        uwriteln!(f, "  rf_rats      = x{}", hex!(self.rf_rats)).ok();
        uwriteln!(f, "  main_irq     = x{}", hex!(self.main_irq)).ok();
        uwriteln!(f, "  fifo_irq     = x{}", hex!(self.fifo_irq)).ok();
        uwriteln!(f, "  aux_irq      = x{}", hex!(self.aux_irq)).ok();
        uwriteln!(f, "  main_irq_mask= x{}", hex!(self.main_irq_mask)).ok();
        uwriteln!(f, "  fifo_irq_mask= x{}", hex!(self.fifo_irq_mask)).ok();
        uwriteln!(f, "  aux_irq_mask = x{}", hex!(self.aux_irq_mask)).ok();
        uwriteln!(f, "  nfc_cfg      = x{}", hex!(self.nfc_cfg)).ok();
        uwriteln!(f, "  regu_cfg     = x{}", hex!(self.regu_cfg))
    }
}




impl ufmt::uDisplay for InterruptState {
    fn fmt<W: ?Sized>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite
    {
        use ufmt::uwriteln;

        if self.main != 0 {
            // let count =
            //     if (main & (1 << 4)) != 0 || (main & (1<<5)) != 0 {
            //         fm.read_reg(Register::FifoCount)
            //     } else { 0 };

            uwriteln!(f,"MAIN:").ok();
            if (self.main & (Interrupt::Aux as u8)) != 0 { uwriteln!(f,"  aux_flag").ok(); }
            if (self.main & (Interrupt::Fifo as u8)) != 0 { uwriteln!(f,"  fifo_flag").ok(); }
            if (self.main & (Interrupt::Arbitration as u8)) != 0 { uwriteln!(f,"  arbit_flag").ok(); }
            if (self.main & (Interrupt::TxDone as u8)) != 0 { uwriteln!(f,"  tx_done").ok(); }
            if (self.main & (Interrupt::RxDone as u8)) != 0 { uwriteln!(f,"  rx_done").ok(); }
            if  self.count > 0             { uwriteln!(f,"  c:{}", self.count).ok(); }
            if (self.main & (Interrupt::RxStart as u8)) != 0 { uwriteln!(f,"  rx_start").ok(); }
            if (self.main & (Interrupt::Active as u8)) != 0 { uwriteln!(f,"  active").ok(); }
            if (self.main & (Interrupt::RfPower as u8)) != 0 { uwriteln!(f,"  rf_pwon").ok(); }


        }

        if self.fifo != 0 {
            uwriteln!(f,"FIFO:").ok();
            if (self.fifo & (1 << 0)) != 0 { uwriteln!(f,"  fifo_empty").ok(); }
            if (self.fifo & (1 << 1)) != 0 { uwriteln!(f,"  fifo_full").ok(); }
            if (self.fifo & (1 << 2)) != 0 { uwriteln!(f,"  fifo_ovflow").ok(); }
            if (self.fifo & (1 << 3)) != 0 { uwriteln!(f,"  fifo_wl").ok(); }
        }

        if self.aux != 0 {
            uwriteln!(f,"AUX:").ok();
            if (self.aux & (1 << 3)) != 0 { uwriteln!(f,"  framing_error").ok(); }
            if (self.aux & (1 << 4)) != 0 { uwriteln!(f,"  crc_error").ok(); }
            if (self.aux & (1 << 5)) != 0 { uwriteln!(f,"  parity_error").ok(); }
            if (self.aux & (1 << 6)) != 0 { uwriteln!(f,"  ee_prog_err").ok(); }
            if (self.aux & (1 << 7)) != 0 { uwriteln!(f,"  ee_prog_done").ok(); }
        }

        Ok(())
    }
}

impl<SPI, CS, INT> FM11NC08 <SPI, CS, INT>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
    INT: InputPin,
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

        InterruptState{
            main:main,
            fifo:fifo,
            aux: aux,
            count:count,
        }
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
            regu_cfg:regu_cfg,
            atqa:atqa,
            sak1: sak1,
            sak2: sak2,
            tl: tl,
            t0: t0,
            ta: ta,
            tb: tb,
            tc: tc,
            i2c_addr: i2c_addr,
            nfc_cfg: nfc_cfg,
            rblock_ack: rblock_ack,
            rblock_nack: rblock_nack,
        }
    }
}

