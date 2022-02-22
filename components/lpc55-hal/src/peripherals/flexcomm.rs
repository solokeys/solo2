use core::ops::Deref;

use crate::{
    raw,
    typestates::{
        init_state,
        ClocksSupportFlexcommToken,
        pin::{
            flexcomm::{
                I2c,
                I2s,
                Spi,
                Usart,
            },
        },
    },
    peripherals::syscon,
};


pub type Flexcomm = (
    Flexcomm0,
    Flexcomm1,
    Flexcomm2,
    Flexcomm3,
    Flexcomm4,
    Flexcomm5,
    Flexcomm6,
    Flexcomm7,
    Flexcomm8,
);

macro_rules! flexcomm {
    ($fc_hal:ident, $i2c_hal:ident, $i2s_hal:ident, $spi_hal:ident, $usart_hal:ident,
     $fc_pac:ident, $i2c_pac:ident, $i2s_pac:ident, $spi_pac:ident, $usart_pac:ident,
     $register_sel:ident
    ) => {
        pub struct $fc_hal<State = init_state::Unknown> {
            pub(crate) raw_fc: raw::$fc_pac,
            pub(crate) raw_i2c: raw::$i2c_pac,
            pub(crate) raw_i2s: raw::$i2s_pac,
            pub(crate) raw_spi: raw::$spi_pac,
            pub(crate) raw_usart: raw::$usart_pac,
            pub _state: State,
        }

        pub struct $i2c_hal<State = init_state::Enabled> {
            pub(crate) _raw_fc: raw::$fc_pac,
            #[allow(dead_code)]
            pub(crate) raw: raw::$i2c_pac,
            pub(crate) _raw_i2s: raw::$i2s_pac,
            pub(crate) _raw_spi: raw::$spi_pac,
            pub(crate) _raw_usart: raw::$usart_pac,
            pub _state: State,
        }

        impl Deref for $i2c_hal {
            type Target = raw::i2c0::RegisterBlock;
            fn deref(&self) -> &Self::Target {
                &self.raw
            }
        }

        impl I2c for $i2c_hal {}

        pub struct $i2s_hal<State = init_state::Enabled> {
            pub(crate) _raw_fc: raw::$fc_pac,
            pub(crate) _raw_i2c: raw::$i2c_pac,
            #[allow(dead_code)]
            pub(crate) raw: raw::$i2s_pac,
            pub(crate) _raw_spi: raw::$spi_pac,
            pub(crate) _raw_usart: raw::$usart_pac,
            pub _state: State,
        }

        impl I2s for $i2s_hal {}

        pub struct $spi_hal<State = init_state::Enabled> {
            pub(crate) _raw_fc: raw::$fc_pac,
            pub(crate) _raw_i2c: raw::$i2c_pac,
            pub(crate) _raw_i2s: raw::$i2s_pac,
            #[allow(dead_code)]
            pub(crate) raw: raw::$spi_pac,
            pub(crate) _raw_usart: raw::$usart_pac,
            pub _state: State,
        }

        impl Deref for $spi_hal {
            type Target = raw::spi0::RegisterBlock;
            fn deref(&self) -> &Self::Target {
                &self.raw
            }
        }

        impl Spi for $spi_hal {}

        pub struct $usart_hal<State = init_state::Enabled> {
            pub(crate) _raw_fc: raw::$fc_pac,
            pub(crate) _raw_i2c: raw::$i2c_pac,
            pub(crate) _raw_i2s: raw::$i2s_pac,
            pub(crate) _raw_spi: raw::$spi_pac,
            #[allow(dead_code)]
            pub(crate) raw: raw::$usart_pac,
            pub _state: State,
        }

        impl Deref for $usart_hal {
            type Target = raw::usart0::RegisterBlock;
            fn deref(&self) -> &Self::Target {
                &self.raw
            }
        }

        impl Usart for $usart_hal {}

        impl core::convert::From<(raw::$fc_pac, raw::$i2c_pac, raw::$i2s_pac, raw::$spi_pac, raw::$usart_pac)> for $fc_hal {
            fn from(raw: (raw::$fc_pac, raw::$i2c_pac, raw::$i2s_pac, raw::$spi_pac, raw::$usart_pac)) -> Self {
                $fc_hal::new(raw)
            }
        }

        impl $fc_hal {
            fn new(raw: (raw::$fc_pac, raw::$i2c_pac, raw::$i2s_pac, raw::$spi_pac, raw::$usart_pac)) -> Self {
                $fc_hal {
                    raw_fc: raw.0,
                    raw_i2c: raw.1,
                    raw_i2s: raw.2,
                    raw_spi: raw.3,
                    raw_usart: raw.4,
                    _state: init_state::Unknown,
                }
            }

            // pub unsafe fn steal() -> Self {
            //     // seems a little wastefule to steal the full peripherals but ok..
            //     Self::new(raw::Peripherals::steal().$pac_name)
            // }
        }

        impl<State> $fc_hal<State> {
            pub fn release(self) -> (raw::$fc_pac, raw::$i2c_pac, raw::$i2s_pac, raw::$spi_pac, raw::$usart_pac) {
                (self.raw_fc, self.raw_i2c, self.raw_i2s, self.raw_spi, self.raw_usart)
            }
        }

        impl $fc_hal {
            fn enabled(&mut self, syscon: &mut syscon::Syscon) {
                syscon.reset(&mut self.raw_fc);
                syscon.enable_clock(&mut self.raw_fc);
            }

            pub fn enabled_as_i2c(
                mut self,
                syscon: &mut syscon::Syscon,
                _clocks_token: &ClocksSupportFlexcommToken,
            ) -> $i2c_hal<init_state::Enabled> {

                // The FRG output frequency must not be higher than 48 MHz for SPI and I2S
                // and not higher than 44 MHz for USART and I2C.
                //
                // Currently, we just use the 12MHz clock

                syscon.raw.$register_sel().modify(|_, w| w.sel().enum_0x2()); // Fro12MHz

                self.enabled(syscon);

                self.raw_fc.pselid.modify(|_, w| w
                    // select I2C function on corresponding FLEXCOMM
                    .persel().i2c()
                    // lock it
                    .lock().locked()
                );
                assert!(self.raw_fc.pselid.read().i2cpresent().is_present());

                $i2c_hal {
                    _raw_fc: self.raw_fc,
                    raw: self.raw_i2c,
                    _raw_i2s: self.raw_i2s,
                    _raw_spi: self.raw_spi,
                    _raw_usart: self.raw_usart,
                    _state: init_state::Enabled(()),
                }
            }

            pub fn enabled_as_spi(
                mut self,
                syscon: &mut syscon::Syscon,
                _clocks_token: &ClocksSupportFlexcommToken,
            ) -> $spi_hal<init_state::Enabled> {

                // The FRG output frequency must not be higher than 48 MHz for SPI and I2S
                // and not higher than 44 MHz for USART and I2C.
                //
                // Currently, we just use the 12MHz clock

                syscon.raw.$register_sel().modify(|_, w| w.sel().enum_0x2()); // Fro12MHz

                self.enabled(syscon);

                self.raw_fc.pselid.modify(|_, w| w
                    // select SPI function on corresponding FLEXCOMM
                    .persel().spi()
                    // lock it
                    .lock().locked()
                );
                assert!(self.raw_fc.pselid.read().spipresent().is_present());

                $spi_hal {
                    _raw_fc: self.raw_fc,
                    _raw_i2c: self.raw_i2c,
                    _raw_i2s: self.raw_i2s,
                    raw: self.raw_spi,
                    _raw_usart: self.raw_usart,
                    _state: init_state::Enabled(()),
                }
            }

            pub fn enabled_as_usart(
                mut self,
                syscon: &mut syscon::Syscon,
                _clocks_token: &ClocksSupportFlexcommToken,
            ) -> $usart_hal<init_state::Enabled> {

                // The FRG output frequency must not be higher than 48 MHz for SPI and I2S
                // and not higher than 44 MHz for USART and I2C.
                //
                // Currently, we just use the 12MHz clock

                syscon.raw.$register_sel().modify(|_, w| w.sel().enum_0x2()); // Fro12MHz

                self.enabled(syscon);

                self.raw_fc.pselid.modify(|_, w| w
                    // select USART function on corresponding FLEXCOMM
                    .persel().usart()
                    // lock it
                    .lock().locked()
                );
                assert!(self.raw_fc.pselid.read().usartpresent().is_present());

                $usart_hal {
                    _raw_fc: self.raw_fc,
                    _raw_i2c: self.raw_i2c,
                    _raw_i2s: self.raw_i2s,
                    _raw_spi: self.raw_spi,
                    raw: self.raw_usart,
                    _state: init_state::Enabled(()),
                }
            }
        }
    }
}

flexcomm!(Flexcomm0, I2c0, I2s0, Spi0, Usart0, FLEXCOMM0, I2C0, I2S0, SPI0, USART0, fcclksel0);
flexcomm!(Flexcomm1, I2c1, I2s1, Spi1, Usart1, FLEXCOMM1, I2C1, I2S1, SPI1, USART1, fcclksel1);
flexcomm!(Flexcomm2, I2c2, I2s2, Spi2, Usart2, FLEXCOMM2, I2C2, I2S2, SPI2, USART2, fcclksel2);
flexcomm!(Flexcomm3, I2c3, I2s3, Spi3, Usart3, FLEXCOMM3, I2C3, I2S3, SPI3, USART3, fcclksel3);
flexcomm!(Flexcomm4, I2c4, I2s4, Spi4, Usart4, FLEXCOMM4, I2C4, I2S4, SPI4, USART4, fcclksel4);
flexcomm!(Flexcomm5, I2c5, I2s5, Spi5, Usart5, FLEXCOMM5, I2C5, I2S5, SPI5, USART5, fcclksel5);
flexcomm!(Flexcomm6, I2c6, I2s6, Spi6, Usart6, FLEXCOMM6, I2C6, I2S6, SPI6, USART6, fcclksel6);
flexcomm!(Flexcomm7, I2c7, I2s7, Spi7, Usart7, FLEXCOMM7, I2C7, I2S7, SPI7, USART7, fcclksel7);

pub struct Flexcomm8<State = init_state::Unknown> {
    pub(crate) raw_fc: raw::FLEXCOMM8,
    pub(crate) raw_spi: raw::SPI8,
    pub _state: State,
}

pub struct Spi8<State = init_state::Enabled> {
    pub(crate) _raw_fc: raw::FLEXCOMM8,
    #[allow(dead_code)]
    pub(crate) raw: raw::SPI8,
    pub _state: State,
}

impl Deref for Spi8 {
    type Target = raw::spi0::RegisterBlock;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl Spi for Spi8 {}

impl core::convert::From<(raw::FLEXCOMM8, raw::SPI8)> for Flexcomm8 {
    fn from(raw: (raw::FLEXCOMM8, raw::SPI8)) -> Self {
        Flexcomm8::new(raw)
    }
}

impl Flexcomm8 {
    fn new(raw: (raw::FLEXCOMM8, raw::SPI8)) -> Self {
        Flexcomm8 {
            raw_fc: raw.0,
            raw_spi: raw.1,
            _state: init_state::Unknown,
        }
    }
}

impl<State> Flexcomm8<State> {
    pub fn release(self) -> (raw::FLEXCOMM8, raw::SPI8) {
        (self.raw_fc, self.raw_spi)
    }
}

impl Flexcomm8 {
    fn enabled(&mut self, syscon: &mut syscon::Syscon) {
        syscon.reset(&mut self.raw_fc);
        syscon.enable_clock(&mut self.raw_fc);
    }

    pub fn enabled_as_spi(
        mut self,
        syscon: &mut syscon::Syscon,
        _clocks_token: &ClocksSupportFlexcommToken,
    ) -> Spi8<init_state::Enabled> {

        // NB: This is the high-speed SPI

        // The FRG output frequency must not be higher than 48 MHz for SPI and I2S
        // and not higher than 44 MHz for USART and I2C.
        //
        // Currently, we just use the 12MHz clock

        syscon.raw.hslspiclksel.modify(|_, w| w.sel().enum_0x2()); // Fro12MHz

        self.enabled(syscon);

        self.raw_fc.pselid.modify(|_, w| w
            // select SPI function on corresponding FLEXCOMM
            .persel().spi()
            // lock it
            .lock().locked()
        );
        assert!(self.raw_fc.pselid.read().spipresent().is_present());

        Spi8 {
            _raw_fc: self.raw_fc,
            raw: self.raw_spi,
            _state: init_state::Enabled(()),
        }
    }

}
