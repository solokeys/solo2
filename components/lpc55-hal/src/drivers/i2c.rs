use core::marker::PhantomData;
use core::cmp::min;
use crate::time::Hertz;
use crate::traits::wg::blocking::i2c::{
    Read,
    Write,
    WriteRead,
};
use crate::typestates::pin::{
    flexcomm::{
        // Trait marking I2C peripherals and pins
        I2c,
        I2cPins,
    },
    PinId,
};

pub mod prelude {
    pub use super::I2cMaster;
    pub use super::Error as I2cError;
    pub use super::Result as I2cResult;
}

/// I2C error
#[derive(Debug)]
pub enum Error {
    /// Bus error (catch-all)
    Bus,
    /// Arbitration loss
    ArbitrationLoss,
    /// NACK
    NackAddress,
    /// NACK
    NackData,
    /// Start/Stop error
    StartStop,

    #[doc(hidden)]
    _Extensible,
}

pub type Result<T> = core::result::Result<T, Error>;


// TODO: Parametrize with Master/Slave MODE
/// I2C peripheral operating in master mode
pub struct I2cMaster<PIO1, PIO2, I2C, PINS>
where
    PIO1: PinId,
    PIO2: PinId,
    I2C: I2c,
    PINS: I2cPins<PIO1, PIO2, I2C>,
{
    i2c: I2C,
    pins: PINS,
    _pin1: PhantomData<PIO1>,
    _pin2: PhantomData<PIO2>,
}

impl<PIO1, PIO2, I2C, PINS> I2cMaster<PIO1, PIO2, I2C, PINS>
where
    PIO1: PinId,
    PIO2: PinId,
    I2C: I2c,
    PINS: I2cPins<PIO1, PIO2, I2C>,
{
    /// Weird crashes happen when running system at 150Mhz PLL.
    /// Suggested use: 100khz or 400khz
    pub fn new<Speed: Into<Hertz>>(i2c: I2C, pins: PINS, speed: Speed) -> Self {
        // Simplified setup: We always use 12MHz clock, and only support 100kHz
        let speed: Hertz = speed.into();
        let speed: u32 = speed.0;
        assert!(speed <= 1_000_000);
        i2c.cfg.modify(|_, w| w
            .msten().enabled()
            // .slven().disabled()
            // .monen().disabled()
            // ...etc.
        );

        // use cortex_m_semihosting::hprintln;

        // logic from `fsl_i2c.c` in SDK
        let mut best_div: u16 = 0;
        let mut best_scl: u8 = 0;
        let mut best_err: u32 = 0;
        for scl in (2..=9).rev() {
            let denominator = 2 * scl * speed;
            let div = min(10_000, 12_000_000 / denominator);
            let err = 12_000_000 - div * denominator;
            if err < best_err || best_err == 0 {
                // first time, or smaller error
                best_div = div as u16; // limited by 10_000
                best_scl = scl as u8; // limited by 9
                best_err = err;
            }
            if err == 0 || div >= 10_000 {
                // clamped at 10k means next scl is smaller means err is larger
                break;
            }
        }

        // Weird suggestion from UM.
        // UM also claims both that:
        // - DIV must be 1 (?!)
        // - Frequency after clkdiv must be <= 2 mhz
        //
        // if speed.0 == 400 {
        //     best_div = 14;
        //     best_scl = 0;
        // }

        // hprintln!("speed {}, div {}, scl {}", speed, best_div, best_scl).ok();

        // best_div = 10; best_scl = 6;
        // 100 kbits/s: div = 10, scl = 6
        // 400 kbits/s: div = 3, scl = 5
        // 1 mbit/s: div = 1, scl = 6
        i2c.clkdiv.modify(|_, w| unsafe { w.divval().bits(best_div - 1) } );
        i2c.msttime.modify(|_, w| w
            .mstsclhigh().bits(best_scl - 2)
            .mstscllow().bits(best_scl - 2)
        );

        // or whatever...

        Self {
            i2c,
            pins,
            _pin1: PhantomData,
            _pin2: PhantomData,
        }
    }

    pub fn release(self) -> (I2C, PINS) {
        (self.i2c, self.pins)
    }

    #[inline(always)]
    fn return_on_error(&self) -> Result<()> {
        // use cortex_m_semihosting::dbg;
        if self.i2c.stat.read().mststate().is_nack_data() {
            // dbg!(Error::NackData);
            return Err(Error::NackData);
        }
        if self.i2c.stat.read().mststate().is_nack_address() {
            // dbg!(Error::NackData);
            return Err(Error::NackAddress);
        }
        if self.i2c.stat.read().mstarbloss().is_arbitration_loss() {
            // dbg!(Error::NackData);
            return Err(Error::ArbitrationLoss);
        }
        if self.i2c.stat.read().mstststperr().is_error() {
            // dbg!(Error::NackData);
            return Err(Error::StartStop);
        }
        Ok(())
    }

    fn write_without_stop(&mut self, addr: u8, bytes: &[u8]) -> Result<()> {
        self.return_on_error()?;

        // Write the slave address with the RW bit set to 0 to the master data register MSTDAT.
        self.i2c
            .mstdat
            .modify(|_, w| unsafe { w.data().bits(addr << 1) });
        // Start the transmission by setting the MSTSTART bit to 1 in the master control register.
        self.i2c.mstctl.write(|w| w.mststart().start());
        // Wait for the pending status to be set (MSTPENDING = 1) by polling the STAT register
        // TODO: Consider implementing a timeout (loop at most N times...) :TODO
        while self.i2c.stat.read().mstpending().is_in_progress() {
            continue;
        }

        self.return_on_error()?;
        if !self.i2c.stat.read().mststate().is_transmit_ready() {
            // dbg!(Error::Bus);
            return Err(Error::Bus);
        }

        // Send bytes
        for byte in bytes {
            // write a byte
            self.i2c.mstdat.modify(|_, w| unsafe { w.data().bits(*byte) } );
            // instruct master to continue
            self.i2c.mstctl.write(|w| w.mstcontinue().continue_());
            // Wait until done
            while self.i2c.stat.read().mstpending().is_in_progress() { continue; }

            // Error handling
            self.return_on_error()?;
            if !self.i2c.stat.read().mststate().is_transmit_ready() {
                // dbg!(Error::Bus);
                return Err(Error::Bus);
            }
        }

        // Fallthrough is success
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        // Stop the transmission by setting the MSTSTOP bit to 1 in the master control register.
        self.i2c.mstctl.write(|w| w.mststop().stop());
        // Wait for the pending status to be set (MSTPENDING = 1) by polling the STAT register
        while self.i2c.stat.read().mstpending().is_in_progress() {}

        self.return_on_error()?;
        if !self.i2c.stat.read().mststate().is_idle() {
            // dbg!(Error::Bus);
            return Err(Error::Bus);
        }

        // Fallthrough is success
        Ok(())
    }
}

impl<PIO1, PIO2, I2C, PINS> Write for I2cMaster<PIO1, PIO2, I2C, PINS>
where
    PIO1: PinId,
    PIO2: PinId,
    I2C: I2c,
    PINS: I2cPins<PIO1, PIO2, I2C>,
{
    type Error = Error;

    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<()> {
        self.write_without_stop(addr, bytes)?;
        self.stop()
    }
}

impl<PIO1, PIO2, I2C, PINS> Read for I2cMaster<PIO1, PIO2, I2C, PINS>
where
    PIO1: PinId,
    PIO2: PinId,
    I2C: I2c,
    PINS: I2cPins<PIO1, PIO2, I2C>,
{
    type Error = Error;

    fn read(&mut self, addr: u8, buffer: &mut [u8]) -> Result<()> {
        if let Some((last, buffer)) = buffer.split_last_mut() {

            // Write the slave address with the RW bit set to 1 to the master data register MSTDAT.
            self.i2c.mstdat.modify(|_, w| unsafe { w.data().bits((addr << 1) | 1) } );
            // Start the transmission by setting the MSTSTART bit to 1 in the master control register.
            self.i2c.mstctl.write(|w| w.mststart().start());

            // Wait for the pending status to be set (MSTPENDING = 1) by polling the STAT register
            while self.i2c.stat.read().mstpending().is_in_progress() {}

            self.return_on_error()?;
            if !self.i2c.stat.read().mststate().is_receive_ready() {
                return Err(Error::Bus);
            }


            for byte in buffer {
                // Read a byte
                *byte = self.i2c.mstdat.read().data().bits();
                // Instruct master to continue
                self.i2c.mstctl.write(|w| w.mstcontinue().continue_());

                // Wait for next byte
                while self.i2c.stat.read().mstpending().is_in_progress() {}

                self.return_on_error()?;
                if !self.i2c.stat.read().mststate().is_receive_ready() {
                    return Err(Error::Bus);
                }
            }

            // Read last byte
            *last = self.i2c.mstdat.read().data().bits();

            self.stop()?;
        }

        // Fallthrough is success
        Ok(())
    }
}

impl<PIO1, PIO2, I2C, PINS> WriteRead for I2cMaster<PIO1, PIO2, I2C, PINS>
where
    PIO1: PinId,
    PIO2: PinId,
    I2C: I2c,
    PINS: I2cPins<PIO1, PIO2, I2C>,
{
    type Error = Error;

    fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<()> {
        self.write_without_stop(addr, bytes)?;
        self.read(addr, buffer)?;

        Ok(())
    }
}

        // impl<PINS> I2cCommon for I2cMaster<I2C, PINS>
        // where
        //     PINS: I2cPins<I2C>,
        // {
        //     fn send_byte(&self, byte: u8) -> Result<(), Error> {
        //         // Write the byte
        //         self.i2c.mstdat.modify(|_, w| unsafe { w.data().bits(byte) } );
        //         // Instruct master to continue
        //         self.i2c.mstctl.modify(|_, w| w.mstcontinue().continue_());
        //         // Wait until done
        //         while self.i2c.stat.read().mstpending().is_in_progress() {}
        //         // Check for error
        //         if !self.i2c.stat.read().mststate().is_transmit_ready() {
        //             return Err(Error::Nack);
        //         }

        //         Ok(())
        //     }

        //     fn recv_byte(&self) -> Result<u8, Error> {
        //         let data = self.i2c.mstdat.read().data().bits();
        //         // Wait until done
        //         while self.i2c.stat.read().mstpending().is_in_progress() {}
        //         // Check for error
        //         if !self.i2c.stat.read().mststate().is_receive_ready() {
        //             return Err(Error::Nack);
        //         }
        //         Ok(data)
        //     }
        // }


// macro_rules impl_i2c
// }}

// impl_i2c!(I2c0, I2c0Master);
// impl_i2c!(I2c1, I2c1Master);
// impl_i2c!(I2c2, I2c2Master);
// impl_i2c!(I2c3, I2c3Master);
// impl_i2c!(I2c4, I2c4Master);
// impl_i2c!(I2c5, I2c5Master);
// impl_i2c!(I2c6, I2c6Master);
// impl_i2c!(I2c7, I2c7Master);
