use crate::{
    peripherals::{
        gpio::Gpio,
        iocon::Iocon,
        flexcomm,
        ctimer,
    },
    typestates::{
        init_state,
        pin::{
            function,
            state::{
                self,
                Special,
            },
            // All the I2cSclPin etc. are here
            flexcomm as fc,
            flexcomm::ChipSelect,
        },
    },
};

pub use crate::typestates::pin::gpio::{
    direction,
    Level,
};

// Implements GPIO pins
pub mod gpio;

pub use crate::typestates::pin::{
    PinId,
    PinType,
};

use crate::typestates::reg_proxy::RegClusterProxy;


/// Main API to control for controlling pins:w
pub struct Pin<T: PinId, S: state::PinState> {
    pub(crate) id: T,
    #[allow(dead_code)]
    pub(crate) state: S,
}


impl Pin<Pio0_22, state::Unused> {
    pub fn into_usb0_vbus_pin(
        self,
        iocon: &mut Iocon<init_state::Enabled>,
    ) -> Pin<Pio0_22, state::Special<function::USB0_VBUS>> {
        iocon.raw.pio0_22.modify(|_, w|
            w
            .func().alt7() // FUNC7, pin configured as USB0_VBUS
            .mode().inactive() // MODE_INACT, no additional pin function
            .slew().standard() // SLEW_STANDARD, standard mode, slew rate control is enabled
            .invert().disabled() // INV_DI, input function is not inverted
            .digimode().digital() // DIGITAL_EN, enable digital fucntion
            .od().normal() // OPENDRAIN_DI, open drain is disabled
        );

        Pin {
            id: self.id,
            state: state::Special {
                _function: function::USB0_VBUS,
            },
        }
    }
}


// seems a bit inefficient, but want to be able to safely
// take individual pins instead of the whole bunch
static mut PIN_TAKEN: [[bool; 32]; 2] = [[false; 32]; 2];

macro_rules! pins {
    ($(
        $field:ident,
        $pin:ident,
        $port:expr,
        $number:expr,
        $type:expr,
        $default_state_ty:ty,
        $default_state_val:expr;
    )*) => {
        /// Provides access to all pins
        #[allow(missing_docs)]
        pub struct Pins {
            $(pub $field: Pin<$pin, $default_state_ty>,)*
        }

        impl Pins {

            fn any_taken() -> bool {
                unsafe {
                    let any_port_0 = PIN_TAKEN[0].iter().any(|x| *x);
                    let any_port_1 = PIN_TAKEN[1].iter().any(|x| *x);
                    any_port_0 || any_port_1
                }
            }

            fn set_all_taken() {
                unsafe {
                    for entry in PIN_TAKEN[0].iter_mut() { *entry = true; }
                    for entry in PIN_TAKEN[1].iter_mut() { *entry = true; }
                }
            }

            fn set_all_released() {
                unsafe {
                    for entry in PIN_TAKEN[0].iter_mut() { *entry = false; }
                    for entry in PIN_TAKEN[1].iter_mut() { *entry = false; }
                }
            }

            pub fn take() -> Option<Self> {
                if Self::any_taken() {
                    None
                } else {
                    Some(unsafe {
                        Self::set_all_taken();
                        Self::steal()
                    } )
                }
            }

            pub fn release(self) {
                Self::set_all_released();
            }

            pub unsafe fn steal() -> Self {
                Self {
                    $(
                        $field: $pin::steal(),
                    )*
                }
            }
        }


        $(
            /// Identifies a specific pin
            ///
            /// Pins can be `take`n individually, or en bloc via `Pins`.
            #[allow(non_camel_case_types)]
            pub struct $pin(());

            impl Pin<$pin, state::Unused>  {
                /// Transition pin to GPIO state
                pub fn into_gpio_pin(
                    self,
                    iocon: &mut Iocon<init_state::Enabled>,
                    _: &mut Gpio<init_state::Enabled>,
                ) -> Pin<$pin, state::Gpio<direction::Unknown>> {
                    // TODO: need to set FUNC to 0 at minimum
                    iocon.raw.$field.modify(|_, w| w
                        .func().alt0() // FUNC $i, pin configured as $FUNCTION
                        .mode().inactive() // MODE_INACT, no additional pin function
                        .slew().standard() // SLEW_STANDARD, standard mode, slew rate control is enabled
                        .invert().disabled() // INV_DI, input function is not inverted
                        .digimode().digital() // DIGITAL_EN, enable digital fucntion
                        .od().normal() // OPENDRAIN_DI, open drain is disabled
                    );
                    Pin {
                        id: self.id,
                        state: state::Gpio {
                            // b: RegClusterProxy::new(),
                            // w: RegClusterProxy::new(),
                            dirset: RegClusterProxy::new(),
                            dirclr: RegClusterProxy::new(),
                            pin: RegClusterProxy::new(),
                            set: RegClusterProxy::new(),
                            clr: RegClusterProxy::new(),

                            _direction: direction::Unknown,
                        },
                    }
                }


            }

            impl $pin {
                pub fn take() -> Option<Pin<Self, $default_state_ty>> {
                    if unsafe { PIN_TAKEN[$port][$number] } {
                        None
                    } else {
                        Some(unsafe {
                            Self::steal()
                        } )
                    }
                }

                pub fn release(self) {
                    unsafe { PIN_TAKEN[$port][$number] = false; }
                }

                pub unsafe fn steal() -> Pin<Self, $default_state_ty> {
                    PIN_TAKEN[$port][$number] = true;
                    Pin {
                        id: Self(()),
                        state: $default_state_val,
                    }
                }
            }

            impl PinId for $pin {
                const PORT: usize = $port;
                const NUMBER: u8 = $number;
                const MASK: u32 = 0x1 << $number;
                const OFFSET: usize = (0x20 << $port) + (0x1 << $number);
                const TYPE: PinType = $type;
            }
        )*
    }
}

macro_rules! analog_pins {
    ($(
        $field:ident,
        $pin:ident,
        $port:expr,
        $number:expr,
        $type:expr,
        $default_state_ty:ty,
        $default_state_val:expr,
        $channel:expr;
    )*) => {
        /// Transition pin to Analog input
        $(
            impl Pin<$pin, state::Unused>  {
                pub fn into_analog_input(
                    self,
                    iocon: &mut Iocon<init_state::Enabled>,
                    _: &mut Gpio<init_state::Enabled>,
                ) -> Pin<$pin, state::Analog<direction::Input>> {

                    // TODO: need to set FUNC to 0 at minimum
                    iocon.raw.$field.modify(|_, w| w
                        .func().alt0() // FUNC $i, pin configured as $FUNCTION
                        .mode().inactive() // MODE_INACT, no additional pin function
                        .slew().standard() // SLEW_STANDARD, standard mode, slew rate control is enabled
                        .invert().disabled() // INV_DI, input function is not inverted
                        .digimode().analog() // DIGITAL_EN, enable digital fucntion
                        .od().normal() // OPENDRAIN_DI, open drain is disabled
                        .asw().set_bit() // ASW, analog input enabled
                    );

                    // self.state.dirclr[T::PORT].write(|w| unsafe { w.dirclrp().bits(T::MASK) });
                    let pin = Pin {
                        id: self.id,
                        state: state::Analog{
                            channel: $channel,
                            dirclr: RegClusterProxy::new(),
                            _direction: direction::Unknown,
                        },
                    };
                    return pin.into_input();
                }
            }
        )*
    }
}

macro_rules! ctimer_match_output_pins {
    ($(
        $ctimer:ty,
        $method:ident,
        $field:ident,
        $pin:ident,
        $func:expr,
        $channel_type:ident,
        $channel_number:expr;
    )*) => {
        /// Transition pin to CTIMER/PWM output
        $(
            impl Pin<$pin, state::Unused>  {
                pub fn $method (
                    self,
                    iocon: &mut Iocon<init_state::Enabled>,
                ) -> Pin<$pin, state::Special<function::$channel_type<$ctimer>>> {

                    // TODO: need to set FUNC to 0 at minimum
                    iocon.raw.$field.modify(|_, w| unsafe { w
                        .func().bits($func) // CMAT function
                        .mode().inactive() // MODE_INACT, no additional pin function
                        .slew().standard() // SLEW_STANDARD, standard mode, slew rate control is enabled
                        .invert().disabled() // INV_DI, input function is not inverted
                        .digimode().digital() // DIGITAL_EN, enable digital function
                        .od().normal() // OPENDRAIN_DI, open drain is disabled
                    });

                    Pin {
                        id: self.id,
                        state: state::Special{
                            _function: function::$channel_type {_marker: core::marker::PhantomData}
                        }
                    }
                }
            }

            impl Pin<$pin, state::Special<function::$channel_type<$ctimer>>> {
                pub const CHANNEL: u8 = $channel_number;
                pub fn get_channel(&self) -> u8 {
                    Self::CHANNEL
                }
            }
        )*
    }
}



pins!(
    pio0_0 , Pio0_0 , 0,  0, PinType::A, state::Unused, state::Unused;
    pio0_1 , Pio0_1 , 0,  1, PinType::D, state::Unused, state::Unused;
    pio0_2 , Pio0_2 , 0,  2, PinType::D, state::Unused, state::Unused;
    pio0_3 , Pio0_3 , 0,  3, PinType::D, state::Unused, state::Unused;
    pio0_4 , Pio0_4 , 0,  4, PinType::D, state::Unused, state::Unused;
    pio0_5 , Pio0_5 , 0,  5, PinType::D, state::Unused, state::Unused;
    pio0_6 , Pio0_6 , 0,  6, PinType::D, state::Unused, state::Unused;
    pio0_7 , Pio0_7 , 0,  7, PinType::D, state::Unused, state::Unused;
    pio0_8 , Pio0_8 , 0,  8, PinType::D, state::Unused, state::Unused;
    pio0_9 , Pio0_9 , 0,  9, PinType::A, state::Unused, state::Unused;
    pio0_10, Pio0_10, 0, 10, PinType::A, state::Unused, state::Unused;
    pio0_11, Pio0_11, 0, 11, PinType::A, state::Special<function::SWCLK>,
        state::Special{ _function: function::SWCLK {} };
    pio0_12, Pio0_12, 0, 12, PinType::A, state::Special<function::SWDIO>,
        state::Special{ _function: function::SWDIO {} };
    pio0_13, Pio0_13, 0, 13, PinType::I, state::Unused, state::Unused;
    pio0_14, Pio0_14, 0, 14, PinType::I, state::Unused, state::Unused;
    pio0_15, Pio0_15, 0, 15, PinType::A, state::Unused, state::Unused;
    pio0_16, Pio0_16, 0, 16, PinType::A, state::Unused, state::Unused;
    pio0_17, Pio0_17, 0, 17, PinType::D, state::Unused, state::Unused;
    pio0_18, Pio0_18, 0, 18, PinType::A, state::Unused, state::Unused;
    pio0_19, Pio0_19, 0, 19, PinType::D, state::Unused, state::Unused;
    pio0_20, Pio0_20, 0, 20, PinType::D, state::Unused, state::Unused;
    pio0_21, Pio0_21, 0, 21, PinType::D, state::Unused, state::Unused;
    pio0_22, Pio0_22, 0, 22, PinType::D, state::Unused, state::Unused;
    pio0_23, Pio0_23, 0, 23, PinType::A, state::Unused, state::Unused;
    pio0_24, Pio0_24, 0, 24, PinType::D, state::Unused, state::Unused;
    pio0_25, Pio0_25, 0, 25, PinType::D, state::Unused, state::Unused;
    pio0_26, Pio0_26, 0, 26, PinType::D, state::Unused, state::Unused;
    pio0_27, Pio0_27, 0, 27, PinType::D, state::Unused, state::Unused;
    pio0_28, Pio0_28, 0, 28, PinType::D, state::Unused, state::Unused;
    pio0_29, Pio0_29, 0, 29, PinType::D, state::Unused, state::Unused;
    pio0_30, Pio0_30, 0, 30, PinType::D, state::Unused, state::Unused;
    pio0_31, Pio0_31, 0, 31, PinType::A, state::Unused, state::Unused;

    pio1_0 , Pio1_0 , 1,  0, PinType::A, state::Unused, state::Unused;
    pio1_1 , Pio1_1 , 1,  1, PinType::D, state::Unused, state::Unused;
    pio1_2 , Pio1_2 , 1,  2, PinType::D, state::Unused, state::Unused;
    pio1_3 , Pio1_3 , 1,  3, PinType::D, state::Unused, state::Unused;
    pio1_4 , Pio1_4 , 1,  4, PinType::D, state::Unused, state::Unused;
    pio1_5 , Pio1_5 , 1,  5, PinType::D, state::Unused, state::Unused;
    pio1_6 , Pio1_6 , 1,  6, PinType::D, state::Unused, state::Unused;
    pio1_7 , Pio1_7 , 1,  7, PinType::D, state::Unused, state::Unused;
    pio1_8 , Pio1_8 , 1,  8, PinType::A, state::Unused, state::Unused;
    pio1_9 , Pio1_9 , 1,  9, PinType::A, state::Unused, state::Unused;
    pio1_10, Pio1_10, 1, 10, PinType::D, state::Unused, state::Unused;
    pio1_11, Pio1_11, 1, 11, PinType::D, state::Unused, state::Unused;
    pio1_12, Pio1_12, 1, 12, PinType::D, state::Unused, state::Unused;
    pio1_13, Pio1_13, 1, 13, PinType::D, state::Unused, state::Unused;
    pio1_14, Pio1_14, 1, 14, PinType::A, state::Unused, state::Unused;
    pio1_15, Pio1_15, 1, 15, PinType::D, state::Unused, state::Unused;
    pio1_16, Pio1_16, 1, 16, PinType::D, state::Unused, state::Unused;
    pio1_17, Pio1_17, 1, 17, PinType::D, state::Unused, state::Unused;
    pio1_18, Pio1_18, 1, 18, PinType::D, state::Unused, state::Unused;
    pio1_19, Pio1_19, 1, 19, PinType::A, state::Unused, state::Unused;
    pio1_20, Pio1_20, 1, 20, PinType::D, state::Unused, state::Unused;
    pio1_21, Pio1_21, 1, 21, PinType::D, state::Unused, state::Unused;
    pio1_22, Pio1_22, 1, 22, PinType::D, state::Unused, state::Unused;
    pio1_23, Pio1_23, 1, 23, PinType::D, state::Unused, state::Unused;
    pio1_24, Pio1_24, 1, 24, PinType::D, state::Unused, state::Unused;
    pio1_25, Pio1_25, 1, 25, PinType::D, state::Unused, state::Unused;
    pio1_26, Pio1_26, 1, 26, PinType::D, state::Unused, state::Unused;
    pio1_27, Pio1_27, 1, 27, PinType::D, state::Unused, state::Unused;
    pio1_28, Pio1_28, 1, 28, PinType::D, state::Unused, state::Unused;
    pio1_29, Pio1_29, 1, 29, PinType::D, state::Unused, state::Unused;
    pio1_30, Pio1_30, 1, 30, PinType::D, state::Unused, state::Unused;
    pio1_31, Pio1_31, 1, 31, PinType::D, state::Unused, state::Unused;
);

analog_pins!(
    pio0_0 , Pio0_0 , 0,  0, PinType::A, state::Unused, state::Unused, 0u8;     // A = 0, B = 1, ...
    pio0_9 , Pio0_9 , 0,  9, PinType::A, state::Unused, state::Unused, 1u8;
    pio0_10, Pio0_10, 0, 10, PinType::A, state::Unused, state::Unused, 1u8;
    pio0_11, Pio0_11, 0, 11, PinType::A, state::Special<function::SWCLK>,
        state::Special{ _function: function::SWCLK {} }, 9u8;
    pio0_12, Pio0_12, 0, 12, PinType::A, state::Special<function::SWDIO>,
        state::Special{ _function: function::SWDIO {} }, 10u8;
    pio0_15, Pio0_15, 0, 15, PinType::A, state::Unused, state::Unused, 2u8;
    pio0_16, Pio0_16, 0, 16, PinType::A, state::Unused, state::Unused, 8u8;
    pio0_18, Pio0_18, 0, 18, PinType::A, state::Unused, state::Unused, 2u8;
    pio0_23, Pio0_23, 0, 23, PinType::A, state::Unused, state::Unused, 0u8;
    pio0_31, Pio0_31, 0, 31, PinType::A, state::Unused, state::Unused, 3u8;

    pio1_0 , Pio1_0 , 1,  0, PinType::A, state::Unused, state::Unused, 11u8;
    pio1_8 , Pio1_8 , 1,  8, PinType::A, state::Unused, state::Unused, 4u8;
    pio1_9 , Pio1_9 , 1,  9, PinType::A, state::Unused, state::Unused, 12u8;
    pio1_14, Pio1_14, 1, 14, PinType::A, state::Unused, state::Unused, 3u8;
    pio1_19, Pio1_19, 1, 19, PinType::A, state::Unused, state::Unused, 0xffu8;   // ACMP_ref
);

ctimer_match_output_pins!(
    ctimer::Ctimer1<init_state::Enabled>, into_match_output, pio1_16 , Pio1_16, 3, MATCH_OUTPUT3, 3;
    ctimer::Ctimer3<init_state::Enabled>, into_match_output, pio0_5 , Pio0_5, 3, MATCH_OUTPUT0, 0;
    ctimer::Ctimer3<init_state::Enabled>, into_match_output, pio1_21 , Pio1_21, 3, MATCH_OUTPUT2, 2;
    ctimer::Ctimer3<init_state::Enabled>, into_match_output, pio1_19 , Pio1_19, 3, MATCH_OUTPUT1, 1;

    ctimer::Ctimer2<init_state::Enabled>, into_match_output, pio1_7, Pio1_7, 3, MATCH_OUTPUT2, 2;
    ctimer::Ctimer2<init_state::Enabled>, into_match_output, pio1_6, Pio1_6, 3, MATCH_OUTPUT1, 1;
    ctimer::Ctimer2<init_state::Enabled>, into_match_output, pio1_4, Pio1_4, 3, MATCH_OUTPUT1, 1;
);

macro_rules! special_pins {
    ($(
        ($Pin:ty,$pin:ident): {
            $(
                ($alt_func:expr, $SPECIAL_FUNCTION:ident): [
                    $(
                        ($method:ident,$Peripheral:ty,$Marker:ident),
                    )*
                ]
            )+
    })*) => {

    $($($(
        impl Pin<$Pin, state::Unused> {
            pub fn $method(
                self,
                iocon: &mut Iocon<init_state::Enabled>,
            ) ->Pin<$Pin, state::Special<function::$SPECIAL_FUNCTION>> {
                // unfortunately, data sheet has more FUNCs than SVD has alts
                // otherwise, it would be safe
                iocon.raw.$pin.modify(|_, w| unsafe {
                    w
                    .func().bits($alt_func) // FUNC $i, pin configured as $FUNCTION
                    .mode().inactive() // MODE_INACT, no additional pin function
                    .slew().standard() // SLEW_STANDARD, standard mode, slew rate control is enabled
                    .invert().disabled() // INV_DI, input function is not inverted
                    .digimode().digital() // DIGITAL_EN, enable digital function
                    .od().normal() // OPENDRAIN_DI, open drain is disabled
                });

                Pin {
                    id: self.id,
                    state: Special {
                        _function: function::$SPECIAL_FUNCTION {},

                    },
                }
            }
        }
    )*)+)*
    }
}

///////////////////////////////////////////////////////////////////////////////
// all that follows is generated with `scripts/extract-flexcomm-data.py`
// NB: Pio0_13 and Pio0_14 have a repetition of methods, manually commented out
// Note also that these two are precisely the specialized I2C pins.
///////////////////////////////////////////////////////////////////////////////

// TODO: remove the $Marker argument
special_pins!{
    (Pio0_0, pio0_0): {
        (2, FC3_SCK): [
            (into_usart3_sclk_pin, Usart3, UsartSclkPin),
            (into_spi3_sck_pin, Spi3, SpiSckPin),
        ]
    }
    (Pio0_1, pio0_1): {
        (2, FC3_CTS_SDA_SSEL0): [
            (into_usart3_cts_pin, Usart3, UsartCtsPin),
            (into_i2c3_sda_pin, I2c3, I2cSdaPin),
            (into_spi3_cs_pin, Spi3, SpiCsPin),
        ]
    }
    (Pio0_2, pio0_2): {
        (1, FC3_TXD_SCL_MISO_WS): [
            (into_usart3_tx_pin, Usart3, UsartTxPin),
            (into_i2c3_scl_pin, I2c3, I2cSclPin),
            (into_spi3_miso_pin, Spi3, SpiMisoPin),
            (into_i2s3_ws_pin, I2s3, I2sWsPin),
        ]
    }
    (Pio0_3, pio0_3): {
        (1, FC3_RXD_SDA_MOSI_DATA): [
            (into_usart3_rx_pin, Usart3, UsartRxPin),
            (into_i2c3_sda_pin, I2c3, I2cSdaPin),
            (into_spi3_mosi_pin, Spi3, SpiMosiPin),
            (into_i2s3_sda_pin, I2s3, I2sSdaPin),
        ]
    }
    (Pio0_4, pio0_4): {
        (2, FC4_SCK): [
            (into_usart4_sclk_pin, Usart4, UsartSclkPin),
            (into_spi4_sck_pin, Spi4, SpiSckPin),
        ]
    }
    (Pio0_5, pio0_5): {
        (2, FC4_RXD_SDA_MOSI_DATA): [
            (into_usart4_rx_pin, Usart4, UsartRxPin),
            (into_i2c4_sda_pin, I2c4, I2cSdaPin),
            (into_spi4_mosi_pin, Spi4, SpiMosiPin),
            (into_i2s4_sda_pin, I2s4, I2sSdaPin),
        ]
    }
    (Pio0_5, pio0_5): {
        (8, FC3_RTS_SCL_SSEL1): [
            (into_usart3_rts_pin, Usart3, UsartRtsPin),
            (into_i2c3_scl_pin, I2c3, I2cSclPin),
            (into_spi3_cs_pin, Spi3, SpiCsPin),
        ]
    }
    (Pio0_6, pio0_6): {
        (1, FC3_SCK): [
            (into_usart3_sclk_pin, Usart3, UsartSclkPin),
            (into_spi3_sck_pin, Spi3, SpiSckPin),
        ]
    }
    (Pio0_7, pio0_7): {
        (1, FC3_RTS_SCL_SSEL1): [
            (into_usart3_rts_pin, Usart3, UsartRtsPin),
            (into_i2c3_scl_pin, I2c3, I2cSclPin),
            (into_spi3_cs_pin, Spi3, SpiCsPin),
        ]
    }
    (Pio0_7, pio0_7): {
        (3, FC5_SCK): [
            (into_usart5_sclk_pin, Usart5, UsartSclkPin),
            (into_spi5_sck_pin, Spi5, SpiSckPin),
        ]
    }
    (Pio0_7, pio0_7): {
        (4, FC1_SCK): [
            (into_usart1_sclk_pin, Usart1, UsartSclkPin),
            (into_spi1_sck_pin, Spi1, SpiSckPin),
        ]
    }
    (Pio0_8, pio0_8): {
        (1, FC3_SSEL3): [
            (into_spi3_cs_pin, Spi3, SpiCsPin),
        ]
    }
    (Pio0_8, pio0_8): {
        (3, FC5_RXD_SDA_MOSI_DATA): [
            (into_usart5_rx_pin, Usart5, UsartRxPin),
            (into_i2c5_sda_pin, I2c5, I2cSdaPin),
            (into_spi5_mosi_pin, Spi5, SpiMosiPin),
            (into_i2s5_sda_pin, I2s5, I2sSdaPin),
        ]
    }
    (Pio0_9, pio0_9): {
        (1, FC3_SSEL2): [
            (into_spi3_cs_pin, Spi3, SpiCsPin),
        ]
    }
    (Pio0_9, pio0_9): {
        (3, FC5_TXD_SCL_MISO_WS): [
            (into_usart5_tx_pin, Usart5, UsartTxPin),
            (into_i2c5_scl_pin, I2c5, I2cSclPin),
            (into_spi5_miso_pin, Spi5, SpiMisoPin),
            (into_i2s5_ws_pin, I2s5, I2sWsPin),
        ]
    }
    (Pio0_10, pio0_10): {
        (1, FC6_SCK): [
            (into_usart6_sclk_pin, Usart6, UsartSclkPin),
            (into_spi6_sck_pin, Spi6, SpiSckPin),
        ]
    }
    (Pio0_10, pio0_10): {
        (4, FC1_TXD_SCL_MISO_WS): [
            (into_usart1_tx_pin, Usart1, UsartTxPin),
            (into_i2c1_scl_pin, I2c1, I2cSclPin),
            (into_spi1_miso_pin, Spi1, SpiMisoPin),
            (into_i2s1_ws_pin, I2s1, I2sWsPin),
        ]
    }
    (Pio0_11, pio0_11): {
        (1, FC6_RXD_SDA_MOSI_DATA): [
            (into_usart6_rx_pin, Usart6, UsartRxPin),
            (into_i2c6_sda_pin, I2c6, I2cSdaPin),
            (into_spi6_mosi_pin, Spi6, SpiMosiPin),
            (into_i2s6_sda_pin, I2s6, I2sSdaPin),
        ]
    }
    (Pio0_12, pio0_12): {
        (1, FC3_TXD_SCL_MISO_WS): [
            (into_usart3_tx_pin, Usart3, UsartTxPin),
            (into_i2c3_scl_pin, I2c3, I2cSclPin),
            (into_spi3_miso_pin, Spi3, SpiMisoPin),
            (into_i2s3_ws_pin, I2s3, I2sWsPin),
        ]
    }
    (Pio0_12, pio0_12): {
        (7, FC6_TXD_SCL_MISO_WS): [
            (into_usart6_tx_pin, Usart6, UsartTxPin),
            (into_i2c6_scl_pin, I2c6, I2cSclPin),
            (into_spi6_miso_pin, Spi6, SpiMisoPin),
            (into_i2s6_ws_pin, I2s6, I2sWsPin),
        ]
    }
    (Pio0_13, pio0_13): {
        (1, FC1_CTS_SDA_SSEL0): [
            (into_usart1_cts_pin, Usart1, UsartCtsPin),
            (into_i2c1_sda_pin, I2c1, I2cSdaPin),
            (into_spi1_cs_pin, Spi1, SpiCsPin),
        ]
    }
    (Pio0_13, pio0_13): {
        (5, FC1_RXD_SDA_MOSI_DATA): [
            (into_usart1_rx_pin, Usart1, UsartRxPin),
            // (into_i2c1_sda_pin, I2c1, I2cSdaPin),
            (into_spi1_mosi_pin, Spi1, SpiMosiPin),
            (into_i2s1_sda_pin, I2s1, I2sSdaPin),
        ]
    }
    (Pio0_14, pio0_14): {
        (1, FC1_RTS_SCL_SSEL1): [
            (into_usart1_rts_pin, Usart1, UsartRtsPin),
            (into_i2c1_scl_pin, I2c1, I2cSclPin),
            (into_spi1_cs_pin, Spi1, SpiCsPin),
        ]
    }
    (Pio0_14, pio0_14): {
        (6, FC1_TXD_SCL_MISO_WS): [
            (into_usart1_tx_pin, Usart1, UsartTxPin),
            // (into_i2c1_scl_pin, I2c1, I2cSclPin),
            (into_spi1_miso_pin, Spi1, SpiMisoPin),
            (into_i2s1_ws_pin, I2s1, I2sWsPin),
        ]
    }
    (Pio0_15, pio0_15): {
        (1, FC6_CTS_SDA_SSEL0): [
            (into_usart6_cts_pin, Usart6, UsartCtsPin),
            (into_i2c6_sda_pin, I2c6, I2cSdaPin),
            (into_spi6_cs_pin, Spi6, SpiCsPin),
        ]
    }
    (Pio0_16, pio0_16): {
        (1, FC4_TXD_SCL_MISO_WS): [
            (into_usart4_tx_pin, Usart4, UsartTxPin),
            (into_i2c4_scl_pin, I2c4, I2cSclPin),
            (into_spi4_miso_pin, Spi4, SpiMisoPin),
            (into_i2s4_ws_pin, I2s4, I2sWsPin),
        ]
    }
    (Pio0_17, pio0_17): {
        (1, FC4_SSEL2): [
            (into_spi4_cs_pin, Spi4, SpiCsPin),
        ]
    }
    (Pio0_18, pio0_18): {
        (1, FC4_CTS_SDA_SSEL0): [
            (into_usart4_cts_pin, Usart4, UsartCtsPin),
            (into_i2c4_sda_pin, I2c4, I2cSdaPin),
            (into_spi4_cs_pin, Spi4, SpiCsPin),
        ]
    }
    (Pio0_19, pio0_19): {
        (1, FC4_RTS_SCL_SSEL1): [
            (into_usart4_rts_pin, Usart4, UsartRtsPin),
            (into_i2c4_scl_pin, I2c4, I2cSclPin),
            (into_spi4_cs_pin, Spi4, SpiCsPin),
        ]
    }
    (Pio0_19, pio0_19): {
        (7, FC7_TXD_SCL_MISO_WS): [
            (into_usart7_tx_pin, Usart7, UsartTxPin),
            (into_i2c7_scl_pin, I2c7, I2cSclPin),
            (into_spi7_miso_pin, Spi7, SpiMisoPin),
            (into_i2s7_ws_pin, I2s7, I2sWsPin),
        ]
    }
    (Pio0_20, pio0_20): {
        (1, FC3_CTS_SDA_SSEL0): [
            (into_usart3_cts_pin, Usart3, UsartCtsPin),
            (into_i2c3_sda_pin, I2c3, I2cSdaPin),
            (into_spi3_cs_pin, Spi3, SpiCsPin),
        ]
    }
    (Pio0_20, pio0_20): {
        (7, FC7_RXD_SDA_MOSI_DATA): [
            (into_usart7_rx_pin, Usart7, UsartRxPin),
            (into_i2c7_sda_pin, I2c7, I2cSdaPin),
            (into_spi7_mosi_pin, Spi7, SpiMosiPin),
            (into_i2s7_sda_pin, I2s7, I2sSdaPin),
        ]
    }
    (Pio0_20, pio0_20): {
        (8, HS_SPI_SSEL0): [
            (into_spi8_cs_pin, Spi8, SpiCsPin),
        ]
    }
    (Pio0_21, pio0_21): {
        (1, FC3_RTS_SCL_SSEL1): [
            (into_usart3_rts_pin, Usart3, UsartRtsPin),
            (into_i2c3_scl_pin, I2c3, I2cSclPin),
            (into_spi3_cs_pin, Spi3, SpiCsPin),
        ]
    }
    (Pio0_21, pio0_21): {
        (7, FC7_SCK): [
            (into_usart7_sclk_pin, Usart7, UsartSclkPin),
            (into_spi7_sck_pin, Spi7, SpiSckPin),
        ]
    }
    (Pio0_22, pio0_22): {
        (1, FC6_TXD_SCL_MISO_WS): [
            (into_usart6_tx_pin, Usart6, UsartTxPin),
            (into_i2c6_scl_pin, I2c6, I2cSclPin),
            (into_spi6_miso_pin, Spi6, SpiMisoPin),
            (into_i2s6_ws_pin, I2s6, I2sWsPin),
        ]
    }
    (Pio0_23, pio0_23): {
        (5, FC0_CTS_SDA_SSEL0): [
            (into_usart0_cts_pin, Usart0, UsartCtsPin),
            (into_i2c0_sda_pin, I2c0, I2cSdaPin),
            (into_spi0_cs_pin, Spi0, SpiCsPin),
        ]
    }
    (Pio0_24, pio0_24): {
        (1, FC0_RXD_SDA_MOSI_DATA): [
            (into_usart0_rx_pin, Usart0, UsartRxPin),
            (into_i2c0_sda_pin, I2c0, I2cSdaPin),
            (into_spi0_mosi_pin, Spi0, SpiMosiPin),
            (into_i2s0_sda_pin, I2s0, I2sSdaPin),
        ]
    }
    (Pio0_25, pio0_25): {
        (1, FC0_TXD_SCL_MISO_WS): [
            (into_usart0_tx_pin, Usart0, UsartTxPin),
            (into_i2c0_scl_pin, I2c0, I2cSclPin),
            (into_spi0_miso_pin, Spi0, SpiMisoPin),
            (into_i2s0_ws_pin, I2s0, I2sWsPin),
        ]
    }
    (Pio0_26, pio0_26): {
        (1, FC2_RXD_SDA_MOSI_DATA): [
            (into_usart2_rx_pin, Usart2, UsartRxPin),
            (into_i2c2_sda_pin, I2c2, I2cSdaPin),
            (into_spi2_mosi_pin, Spi2, SpiMosiPin),
            (into_i2s2_sda_pin, I2s2, I2sSdaPin),
        ]
    }
    (Pio0_26, pio0_26): {
        (8, FC0_SCK): [
            (into_usart0_sclk_pin, Usart0, UsartSclkPin),
            (into_spi0_sck_pin, Spi0, SpiSckPin),
        ]
    }
    (Pio0_26, pio0_26): {
        (9, HS_SPI_MOSI): [
            (into_spi8_mosi_pin, Spi8, SpiMosiPin),
        ]
    }
    (Pio0_27, pio0_27): {
        (1, FC2_TXD_SCL_MISO_WS): [
            (into_usart2_tx_pin, Usart2, UsartTxPin),
            (into_i2c2_scl_pin, I2c2, I2cSclPin),
            (into_spi2_miso_pin, Spi2, SpiMisoPin),
            (into_i2s2_ws_pin, I2s2, I2sWsPin),
        ]
    }
    (Pio0_27, pio0_27): {
        (7, FC7_RXD_SDA_MOSI_DATA): [
            (into_usart7_rx_pin, Usart7, UsartRxPin),
            (into_i2c7_sda_pin, I2c7, I2cSdaPin),
            (into_spi7_mosi_pin, Spi7, SpiMosiPin),
            (into_i2s7_sda_pin, I2s7, I2sSdaPin),
        ]
    }
    (Pio0_28, pio0_28): {
        (1, FC0_SCK): [
            (into_usart0_sclk_pin, Usart0, UsartSclkPin),
            (into_spi0_sck_pin, Spi0, SpiSckPin),
        ]
    }
    (Pio0_29, pio0_29): {
        (1, FC0_RXD_SDA_MOSI_DATA): [
            (into_usart0_rx_pin, Usart0, UsartRxPin),
            (into_i2c0_sda_pin, I2c0, I2cSdaPin),
            (into_spi0_mosi_pin, Spi0, SpiMosiPin),
            (into_i2s0_sda_pin, I2s0, I2sSdaPin),
        ]
    }
    (Pio0_30, pio0_30): {
        (1, FC0_TXD_SCL_MISO_WS): [
            (into_usart0_tx_pin, Usart0, UsartTxPin),
            (into_i2c0_scl_pin, I2c0, I2cSclPin),
            (into_spi0_miso_pin, Spi0, SpiMisoPin),
            (into_i2s0_ws_pin, I2s0, I2sWsPin),
        ]
    }
    (Pio0_31, pio0_31): {
        (1, FC0_CTS_SDA_SSEL0): [
            (into_usart0_cts_pin, Usart0, UsartCtsPin),
            (into_i2c0_sda_pin, I2c0, I2cSdaPin),
            (into_spi0_cs_pin, Spi0, SpiCsPin),
        ]
    }
    (Pio1_0, pio1_0): {
        (1, FC0_RTS_SCL_SSEL1): [
            (into_usart0_rts_pin, Usart0, UsartRtsPin),
            (into_i2c0_scl_pin, I2c0, I2cSclPin),
            (into_spi0_cs_pin, Spi0, SpiCsPin),
        ]
    }
    (Pio1_1, pio1_1): {
        (1, FC3_RXD_SDA_MOSI_DATA): [
            (into_usart3_rx_pin, Usart3, UsartRxPin),
            (into_i2c3_sda_pin, I2c3, I2cSdaPin),
            (into_spi3_mosi_pin, Spi3, SpiMosiPin),
            (into_i2s3_sda_pin, I2s3, I2sSdaPin),
        ]
    }
    (Pio1_1, pio1_1): {
        (5, HS_SPI_SSEL1): [
            (into_spi8_cs_pin, Spi8, SpiCsPin),
        ]
    }
    (Pio1_2, pio1_2): {
        (6, HS_SPI_SCK): [
            (into_spi8_sck_pin, Spi8, SpiSckPin),
        ]
    }
    (Pio1_3, pio1_3): {
        (6, HS_SPI_MISO): [
            (into_spi8_miso_pin, Spi8, SpiMisoPin),
        ]
    }
    (Pio1_4, pio1_4): {
        (1, FC0_SCK): [
            (into_usart0_sclk_pin, Usart0, UsartSclkPin),
            (into_spi0_sck_pin, Spi0, SpiSckPin),
        ]
    }
    (Pio1_5, pio1_5): {
        (1, FC0_RXD_SDA_MOSI_DATA): [
            (into_usart0_rx_pin, Usart0, UsartRxPin),
            (into_i2c0_sda_pin, I2c0, I2cSdaPin),
            (into_spi0_mosi_pin, Spi0, SpiMosiPin),
            (into_i2s0_sda_pin, I2s0, I2sSdaPin),
        ]
    }
    (Pio1_6, pio1_6): {
        (1, FC0_TXD_SCL_MISO_WS): [
            (into_usart0_tx_pin, Usart0, UsartTxPin),
            (into_i2c0_scl_pin, I2c0, I2cSclPin),
            (into_spi0_miso_pin, Spi0, SpiMisoPin),
            (into_i2s0_ws_pin, I2s0, I2sWsPin),
        ]
    }
    (Pio1_7, pio1_7): {
        (1, FC0_RTS_SCL_SSEL1): [
            (into_usart0_rts_pin, Usart0, UsartRtsPin),
            (into_i2c0_scl_pin, I2c0, I2cSclPin),
            (into_spi0_cs_pin, Spi0, SpiCsPin),
        ]
    }
    (Pio1_8, pio1_8): {
        (1, FC0_CTS_SDA_SSEL0): [
            (into_usart0_cts_pin, Usart0, UsartCtsPin),
            (into_i2c0_sda_pin, I2c0, I2cSdaPin),
            (into_spi0_cs_pin, Spi0, SpiCsPin),
        ]
    }
    (Pio1_8, pio1_8): {
        (5, FC4_SSEL2): [
            (into_spi4_cs_pin, Spi4, SpiCsPin),
        ]
    }
    (Pio1_9, pio1_9): {
        (2, FC1_SCK): [
            (into_usart1_sclk_pin, Usart1, UsartSclkPin),
            (into_spi1_sck_pin, Spi1, SpiSckPin),
        ]
    }
    (Pio1_9, pio1_9): {
        (5, FC4_CTS_SDA_SSEL0): [
            (into_usart4_cts_pin, Usart4, UsartCtsPin),
            (into_i2c4_sda_pin, I2c4, I2cSdaPin),
            (into_spi4_cs_pin, Spi4, SpiCsPin),
        ]
    }
    (Pio1_10, pio1_10): {
        (2, FC1_RXD_SDA_MOSI_DATA): [
            (into_usart1_rx_pin, Usart1, UsartRxPin),
            (into_i2c1_sda_pin, I2c1, I2cSdaPin),
            (into_spi1_mosi_pin, Spi1, SpiMosiPin),
            (into_i2s1_sda_pin, I2s1, I2sSdaPin),
        ]
    }
    (Pio1_11, pio1_11): {
        (2, FC1_TXD_SCL_MISO_WS): [
            (into_usart1_tx_pin, Usart1, UsartTxPin),
            (into_i2c1_scl_pin, I2c1, I2cSclPin),
            (into_spi1_miso_pin, Spi1, SpiMisoPin),
            (into_i2s1_ws_pin, I2s1, I2sWsPin),
        ]
    }
    (Pio1_12, pio1_12): {
        (2, FC6_SCK): [
            (into_usart6_sclk_pin, Usart6, UsartSclkPin),
            (into_spi6_sck_pin, Spi6, SpiSckPin),
        ]
    }
    (Pio1_12, pio1_12): {
        (5, HS_SPI_SSEL2): [
            (into_spi8_cs_pin, Spi8, SpiCsPin),
        ]
    }
    (Pio1_13, pio1_13): {
        (2, FC6_RXD_SDA_MOSI_DATA): [
            (into_usart6_rx_pin, Usart6, UsartRxPin),
            (into_i2c6_sda_pin, I2c6, I2cSdaPin),
            (into_spi6_mosi_pin, Spi6, SpiMosiPin),
            (into_i2s6_sda_pin, I2s6, I2sSdaPin),
        ]
    }
    (Pio1_14, pio1_14): {
        (4, FC5_CTS_SDA_SSEL0): [
            (into_usart5_cts_pin, Usart5, UsartCtsPin),
            (into_i2c5_sda_pin, I2c5, I2cSdaPin),
            (into_spi5_cs_pin, Spi5, SpiCsPin),
        ]
    }
    (Pio1_15, pio1_15): {
        (4, FC5_RTS_SCL_SSEL1): [
            (into_usart5_rts_pin, Usart5, UsartRtsPin),
            (into_i2c5_scl_pin, I2c5, I2cSclPin),
            (into_spi5_cs_pin, Spi5, SpiCsPin),
        ]
    }
    (Pio1_15, pio1_15): {
        (5, FC4_RTS_SCL_SSEL1): [
            (into_usart4_rts_pin, Usart4, UsartRtsPin),
            (into_i2c4_scl_pin, I2c4, I2cSclPin),
            (into_spi4_cs_pin, Spi4, SpiCsPin),
        ]
    }
    (Pio1_16, pio1_16): {
        (2, FC6_TXD_SCL_MISO_WS): [
            (into_usart6_tx_pin, Usart6, UsartTxPin),
            (into_i2c6_scl_pin, I2c6, I2cSclPin),
            (into_spi6_miso_pin, Spi6, SpiMisoPin),
            (into_i2s6_ws_pin, I2s6, I2sWsPin),
        ]
    }
    (Pio1_17, pio1_17): {
        (3, FC6_RTS_SCL_SSEL1): [
            (into_usart6_rts_pin, Usart6, UsartRtsPin),
            (into_i2c6_scl_pin, I2c6, I2cSclPin),
            (into_spi6_cs_pin, Spi6, SpiCsPin),
        ]
    }
    (Pio1_19, pio1_19): {
        (5, FC4_SCK): [
            (into_usart4_sclk_pin, Usart4, UsartSclkPin),
            (into_spi4_sck_pin, Spi4, SpiSckPin),
        ]
    }
    (Pio1_20, pio1_20): {
        (1, FC7_RTS_SCL_SSEL1): [
            (into_usart7_rts_pin, Usart7, UsartRtsPin),
            (into_i2c7_scl_pin, I2c7, I2cSclPin),
            (into_spi7_cs_pin, Spi7, SpiCsPin),
        ]
    }
    (Pio1_20, pio1_20): {
        (5, FC4_TXD_SCL_MISO_WS): [
            (into_usart4_tx_pin, Usart4, UsartTxPin),
            (into_i2c4_scl_pin, I2c4, I2cSclPin),
            (into_spi4_miso_pin, Spi4, SpiMisoPin),
            (into_i2s4_ws_pin, I2s4, I2sWsPin),
        ]
    }
    (Pio1_21, pio1_21): {
        (1, FC7_CTS_SDA_SSEL0): [
            (into_usart7_cts_pin, Usart7, UsartCtsPin),
            (into_i2c7_sda_pin, I2c7, I2cSdaPin),
            (into_spi7_cs_pin, Spi7, SpiCsPin),
        ]
    }
    (Pio1_21, pio1_21): {
        (5, FC4_RXD_SDA_MOSI_DATA): [
            (into_usart4_rx_pin, Usart4, UsartRxPin),
            (into_i2c4_sda_pin, I2c4, I2cSdaPin),
            (into_spi4_mosi_pin, Spi4, SpiMosiPin),
            (into_i2s4_sda_pin, I2s4, I2sSdaPin),
        ]
    }
    (Pio1_22, pio1_22): {
        (5, FC4_SSEL3): [
            (into_spi4_cs_pin, Spi4, SpiCsPin),
        ]
    }
    (Pio1_23, pio1_23): {
        (1, FC2_SCK): [
            (into_usart2_sclk_pin, Usart2, UsartSclkPin),
            (into_spi2_sck_pin, Spi2, SpiSckPin),
        ]
    }
    (Pio1_23, pio1_23): {
        (5, FC3_SSEL2): [
            (into_spi3_cs_pin, Spi3, SpiCsPin),
        ]
    }
    (Pio1_24, pio1_24): {
        (1, FC2_RXD_SDA_MOSI_DATA): [
            (into_usart2_rx_pin, Usart2, UsartRxPin),
            (into_i2c2_sda_pin, I2c2, I2cSdaPin),
            (into_spi2_mosi_pin, Spi2, SpiMosiPin),
            (into_i2s2_sda_pin, I2s2, I2sSdaPin),
        ]
    }
    (Pio1_24, pio1_24): {
        (5, FC3_SSEL3): [
            (into_spi3_cs_pin, Spi3, SpiCsPin),
        ]
    }
    (Pio1_25, pio1_25): {
        (1, FC2_TXD_SCL_MISO_WS): [
            (into_usart2_tx_pin, Usart2, UsartTxPin),
            (into_i2c2_scl_pin, I2c2, I2cSclPin),
            (into_spi2_miso_pin, Spi2, SpiMisoPin),
            (into_i2s2_ws_pin, I2s2, I2sWsPin),
        ]
    }
    (Pio1_26, pio1_26): {
        (1, FC2_CTS_SDA_SSEL0): [
            (into_usart2_cts_pin, Usart2, UsartCtsPin),
            (into_i2c2_sda_pin, I2c2, I2cSdaPin),
            (into_spi2_cs_pin, Spi2, SpiCsPin),
        ]
    }
    (Pio1_26, pio1_26): {
        (5, HS_SPI_SSEL3): [
            (into_spi8_cs_pin, Spi8, SpiCsPin),
        ]
    }
    (Pio1_27, pio1_27): {
        (1, FC2_RTS_SCL_SSEL1): [
            (into_usart2_rts_pin, Usart2, UsartRtsPin),
            (into_i2c2_scl_pin, I2c2, I2cSclPin),
            (into_spi2_cs_pin, Spi2, SpiCsPin),
        ]
    }
    (Pio1_28, pio1_28): {
        (1, FC7_SCK): [
            (into_usart7_sclk_pin, Usart7, UsartSclkPin),
            (into_spi7_sck_pin, Spi7, SpiSckPin),
        ]
    }
    (Pio1_29, pio1_29): {
        (1, FC7_RXD_SDA_MOSI_DATA): [
            (into_usart7_rx_pin, Usart7, UsartRxPin),
            (into_i2c7_sda_pin, I2c7, I2cSdaPin),
            (into_spi7_mosi_pin, Spi7, SpiMosiPin),
            (into_i2s7_sda_pin, I2s7, I2sSdaPin),
        ]
    }
    (Pio1_30, pio1_30): {
        (1, FC7_TXD_SCL_MISO_WS): [
            (into_usart7_tx_pin, Usart7, UsartTxPin),
            (into_i2c7_scl_pin, I2c7, I2cSclPin),
            (into_spi7_miso_pin, Spi7, SpiMisoPin),
            (into_i2s7_ws_pin, I2s7, I2sWsPin),
        ]
    }
}

impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c0> for Pin<PIO, Special<function::FC0_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c0> for Pin<PIO, Special<function::FC0_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c1> for Pin<PIO, Special<function::FC1_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c1> for Pin<PIO, Special<function::FC1_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c2> for Pin<PIO, Special<function::FC2_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c2> for Pin<PIO, Special<function::FC2_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c3> for Pin<PIO, Special<function::FC3_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c3> for Pin<PIO, Special<function::FC3_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c4> for Pin<PIO, Special<function::FC4_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c4> for Pin<PIO, Special<function::FC4_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c5> for Pin<PIO, Special<function::FC5_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c5> for Pin<PIO, Special<function::FC5_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c6> for Pin<PIO, Special<function::FC6_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c6> for Pin<PIO, Special<function::FC6_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c7> for Pin<PIO, Special<function::FC7_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::I2cSclPin<PIO, flexcomm::I2c7> for Pin<PIO, Special<function::FC7_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c0> for Pin<PIO, Special<function::FC0_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c0> for Pin<PIO, Special<function::FC0_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c1> for Pin<PIO, Special<function::FC1_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c1> for Pin<PIO, Special<function::FC1_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c2> for Pin<PIO, Special<function::FC2_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c2> for Pin<PIO, Special<function::FC2_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c3> for Pin<PIO, Special<function::FC3_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c3> for Pin<PIO, Special<function::FC3_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c4> for Pin<PIO, Special<function::FC4_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c4> for Pin<PIO, Special<function::FC4_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c5> for Pin<PIO, Special<function::FC5_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c5> for Pin<PIO, Special<function::FC5_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c6> for Pin<PIO, Special<function::FC6_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c6> for Pin<PIO, Special<function::FC6_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c7> for Pin<PIO, Special<function::FC7_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::I2cSdaPin<PIO, flexcomm::I2c7> for Pin<PIO, Special<function::FC7_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2sSdaPin<PIO, flexcomm::I2s0> for Pin<PIO, Special<function::FC0_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2sSdaPin<PIO, flexcomm::I2s1> for Pin<PIO, Special<function::FC1_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2sSdaPin<PIO, flexcomm::I2s2> for Pin<PIO, Special<function::FC2_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2sSdaPin<PIO, flexcomm::I2s3> for Pin<PIO, Special<function::FC3_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2sSdaPin<PIO, flexcomm::I2s4> for Pin<PIO, Special<function::FC4_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2sSdaPin<PIO, flexcomm::I2s5> for Pin<PIO, Special<function::FC5_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2sSdaPin<PIO, flexcomm::I2s6> for Pin<PIO, Special<function::FC6_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2sSdaPin<PIO, flexcomm::I2s7> for Pin<PIO, Special<function::FC7_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::I2sWsPin<PIO, flexcomm::I2s0> for Pin<PIO, Special<function::FC0_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2sWsPin<PIO, flexcomm::I2s1> for Pin<PIO, Special<function::FC1_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2sWsPin<PIO, flexcomm::I2s2> for Pin<PIO, Special<function::FC2_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2sWsPin<PIO, flexcomm::I2s3> for Pin<PIO, Special<function::FC3_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2sWsPin<PIO, flexcomm::I2s4> for Pin<PIO, Special<function::FC4_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2sWsPin<PIO, flexcomm::I2s5> for Pin<PIO, Special<function::FC5_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2sWsPin<PIO, flexcomm::I2s6> for Pin<PIO, Special<function::FC6_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::I2sWsPin<PIO, flexcomm::I2s7> for Pin<PIO, Special<function::FC7_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi0> for Pin<PIO, Special<function::FC0_CTS_SDA_SSEL0>> {
    const CS: ChipSelect = ChipSelect::Chip0;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi0> for Pin<PIO, Special<function::FC0_RTS_SCL_SSEL1>> {
    const CS: ChipSelect = ChipSelect::Chip1;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi1> for Pin<PIO, Special<function::FC1_CTS_SDA_SSEL0>> {
    const CS: ChipSelect = ChipSelect::Chip0;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi1> for Pin<PIO, Special<function::FC1_RTS_SCL_SSEL1>> {
    const CS: ChipSelect = ChipSelect::Chip1;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi2> for Pin<PIO, Special<function::FC2_CTS_SDA_SSEL0>> {
    const CS: ChipSelect = ChipSelect::Chip0;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi2> for Pin<PIO, Special<function::FC2_RTS_SCL_SSEL1>> {
    const CS: ChipSelect = ChipSelect::Chip1;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi3> for Pin<PIO, Special<function::FC3_CTS_SDA_SSEL0>> {
    const CS: ChipSelect = ChipSelect::Chip0;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi3> for Pin<PIO, Special<function::FC3_RTS_SCL_SSEL1>> {
    const CS: ChipSelect = ChipSelect::Chip1;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi3> for Pin<PIO, Special<function::FC3_SSEL2>> {
    const CS: ChipSelect = ChipSelect::Chip2;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi3> for Pin<PIO, Special<function::FC3_SSEL3>> {
    const CS: ChipSelect = ChipSelect::Chip3;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi4> for Pin<PIO, Special<function::FC4_CTS_SDA_SSEL0>> {
    const CS: ChipSelect = ChipSelect::Chip0;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi4> for Pin<PIO, Special<function::FC4_RTS_SCL_SSEL1>> {
    const CS: ChipSelect = ChipSelect::Chip1;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi4> for Pin<PIO, Special<function::FC4_SSEL2>> {
    const CS: ChipSelect = ChipSelect::Chip2;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi4> for Pin<PIO, Special<function::FC4_SSEL3>> {
    const CS: ChipSelect = ChipSelect::Chip3;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi5> for Pin<PIO, Special<function::FC5_CTS_SDA_SSEL0>> {
    const CS: ChipSelect = ChipSelect::Chip0;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi5> for Pin<PIO, Special<function::FC5_RTS_SCL_SSEL1>> {
    const CS: ChipSelect = ChipSelect::Chip1;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi6> for Pin<PIO, Special<function::FC6_CTS_SDA_SSEL0>> {
    const CS: ChipSelect = ChipSelect::Chip0;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi6> for Pin<PIO, Special<function::FC6_RTS_SCL_SSEL1>> {
    const CS: ChipSelect = ChipSelect::Chip1;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi7> for Pin<PIO, Special<function::FC7_CTS_SDA_SSEL0>> {
    const CS: ChipSelect = ChipSelect::Chip0;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi7> for Pin<PIO, Special<function::FC7_RTS_SCL_SSEL1>> {
    const CS: ChipSelect = ChipSelect::Chip1;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi8> for Pin<PIO, Special<function::HS_SPI_SSEL0>> {
    const CS: ChipSelect = ChipSelect::Chip0;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi8> for Pin<PIO, Special<function::HS_SPI_SSEL1>> {
    const CS: ChipSelect = ChipSelect::Chip1;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi8> for Pin<PIO, Special<function::HS_SPI_SSEL2>> {
    const CS: ChipSelect = ChipSelect::Chip2;
}
impl<PIO: PinId> fc::SpiCsPin<PIO, flexcomm::Spi8> for Pin<PIO, Special<function::HS_SPI_SSEL3>> {
    const CS: ChipSelect = ChipSelect::Chip3;
}
impl<PIO: PinId> fc::SpiMisoPin<PIO, flexcomm::Spi0> for Pin<PIO, Special<function::FC0_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::SpiMisoPin<PIO, flexcomm::Spi1> for Pin<PIO, Special<function::FC1_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::SpiMisoPin<PIO, flexcomm::Spi2> for Pin<PIO, Special<function::FC2_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::SpiMisoPin<PIO, flexcomm::Spi3> for Pin<PIO, Special<function::FC3_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::SpiMisoPin<PIO, flexcomm::Spi4> for Pin<PIO, Special<function::FC4_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::SpiMisoPin<PIO, flexcomm::Spi5> for Pin<PIO, Special<function::FC5_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::SpiMisoPin<PIO, flexcomm::Spi6> for Pin<PIO, Special<function::FC6_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::SpiMisoPin<PIO, flexcomm::Spi7> for Pin<PIO, Special<function::FC7_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::SpiMisoPin<PIO, flexcomm::Spi8> for Pin<PIO, Special<function::HS_SPI_MISO>> {}
impl<PIO: PinId> fc::SpiMosiPin<PIO, flexcomm::Spi0> for Pin<PIO, Special<function::FC0_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::SpiMosiPin<PIO, flexcomm::Spi1> for Pin<PIO, Special<function::FC1_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::SpiMosiPin<PIO, flexcomm::Spi2> for Pin<PIO, Special<function::FC2_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::SpiMosiPin<PIO, flexcomm::Spi3> for Pin<PIO, Special<function::FC3_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::SpiMosiPin<PIO, flexcomm::Spi4> for Pin<PIO, Special<function::FC4_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::SpiMosiPin<PIO, flexcomm::Spi5> for Pin<PIO, Special<function::FC5_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::SpiMosiPin<PIO, flexcomm::Spi6> for Pin<PIO, Special<function::FC6_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::SpiMosiPin<PIO, flexcomm::Spi7> for Pin<PIO, Special<function::FC7_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::SpiMosiPin<PIO, flexcomm::Spi8> for Pin<PIO, Special<function::HS_SPI_MOSI>> {}
impl<PIO: PinId> fc::SpiSckPin<PIO, flexcomm::Spi0> for Pin<PIO, Special<function::FC0_SCK>> {}
impl<PIO: PinId> fc::SpiSckPin<PIO, flexcomm::Spi1> for Pin<PIO, Special<function::FC1_SCK>> {}
impl<PIO: PinId> fc::SpiSckPin<PIO, flexcomm::Spi2> for Pin<PIO, Special<function::FC2_SCK>> {}
impl<PIO: PinId> fc::SpiSckPin<PIO, flexcomm::Spi3> for Pin<PIO, Special<function::FC3_SCK>> {}
impl<PIO: PinId> fc::SpiSckPin<PIO, flexcomm::Spi4> for Pin<PIO, Special<function::FC4_SCK>> {}
impl<PIO: PinId> fc::SpiSckPin<PIO, flexcomm::Spi5> for Pin<PIO, Special<function::FC5_SCK>> {}
impl<PIO: PinId> fc::SpiSckPin<PIO, flexcomm::Spi6> for Pin<PIO, Special<function::FC6_SCK>> {}
impl<PIO: PinId> fc::SpiSckPin<PIO, flexcomm::Spi7> for Pin<PIO, Special<function::FC7_SCK>> {}
impl<PIO: PinId> fc::SpiSckPin<PIO, flexcomm::Spi8> for Pin<PIO, Special<function::HS_SPI_SCK>> {}
impl<PIO: PinId> fc::UsartCtsPin<PIO, flexcomm::Usart0> for Pin<PIO, Special<function::FC0_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::UsartCtsPin<PIO, flexcomm::Usart1> for Pin<PIO, Special<function::FC1_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::UsartCtsPin<PIO, flexcomm::Usart2> for Pin<PIO, Special<function::FC2_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::UsartCtsPin<PIO, flexcomm::Usart3> for Pin<PIO, Special<function::FC3_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::UsartCtsPin<PIO, flexcomm::Usart4> for Pin<PIO, Special<function::FC4_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::UsartCtsPin<PIO, flexcomm::Usart5> for Pin<PIO, Special<function::FC5_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::UsartCtsPin<PIO, flexcomm::Usart6> for Pin<PIO, Special<function::FC6_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::UsartCtsPin<PIO, flexcomm::Usart7> for Pin<PIO, Special<function::FC7_CTS_SDA_SSEL0>> {}
impl<PIO: PinId> fc::UsartRtsPin<PIO, flexcomm::Usart0> for Pin<PIO, Special<function::FC0_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::UsartRtsPin<PIO, flexcomm::Usart1> for Pin<PIO, Special<function::FC1_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::UsartRtsPin<PIO, flexcomm::Usart2> for Pin<PIO, Special<function::FC2_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::UsartRtsPin<PIO, flexcomm::Usart3> for Pin<PIO, Special<function::FC3_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::UsartRtsPin<PIO, flexcomm::Usart4> for Pin<PIO, Special<function::FC4_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::UsartRtsPin<PIO, flexcomm::Usart5> for Pin<PIO, Special<function::FC5_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::UsartRtsPin<PIO, flexcomm::Usart6> for Pin<PIO, Special<function::FC6_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::UsartRtsPin<PIO, flexcomm::Usart7> for Pin<PIO, Special<function::FC7_RTS_SCL_SSEL1>> {}
impl<PIO: PinId> fc::UsartRxPin<PIO, flexcomm::Usart0> for Pin<PIO, Special<function::FC0_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::UsartRxPin<PIO, flexcomm::Usart1> for Pin<PIO, Special<function::FC1_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::UsartRxPin<PIO, flexcomm::Usart2> for Pin<PIO, Special<function::FC2_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::UsartRxPin<PIO, flexcomm::Usart3> for Pin<PIO, Special<function::FC3_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::UsartRxPin<PIO, flexcomm::Usart4> for Pin<PIO, Special<function::FC4_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::UsartRxPin<PIO, flexcomm::Usart5> for Pin<PIO, Special<function::FC5_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::UsartRxPin<PIO, flexcomm::Usart6> for Pin<PIO, Special<function::FC6_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::UsartRxPin<PIO, flexcomm::Usart7> for Pin<PIO, Special<function::FC7_RXD_SDA_MOSI_DATA>> {}
impl<PIO: PinId> fc::UsartSclkPin<PIO, flexcomm::Usart0> for Pin<PIO, Special<function::FC0_SCK>> {}
impl<PIO: PinId> fc::UsartSclkPin<PIO, flexcomm::Usart1> for Pin<PIO, Special<function::FC1_SCK>> {}
impl<PIO: PinId> fc::UsartSclkPin<PIO, flexcomm::Usart2> for Pin<PIO, Special<function::FC2_SCK>> {}
impl<PIO: PinId> fc::UsartSclkPin<PIO, flexcomm::Usart3> for Pin<PIO, Special<function::FC3_SCK>> {}
impl<PIO: PinId> fc::UsartSclkPin<PIO, flexcomm::Usart4> for Pin<PIO, Special<function::FC4_SCK>> {}
impl<PIO: PinId> fc::UsartSclkPin<PIO, flexcomm::Usart5> for Pin<PIO, Special<function::FC5_SCK>> {}
impl<PIO: PinId> fc::UsartSclkPin<PIO, flexcomm::Usart6> for Pin<PIO, Special<function::FC6_SCK>> {}
impl<PIO: PinId> fc::UsartSclkPin<PIO, flexcomm::Usart7> for Pin<PIO, Special<function::FC7_SCK>> {}
impl<PIO: PinId> fc::UsartTxPin<PIO, flexcomm::Usart0> for Pin<PIO, Special<function::FC0_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::UsartTxPin<PIO, flexcomm::Usart1> for Pin<PIO, Special<function::FC1_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::UsartTxPin<PIO, flexcomm::Usart2> for Pin<PIO, Special<function::FC2_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::UsartTxPin<PIO, flexcomm::Usart3> for Pin<PIO, Special<function::FC3_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::UsartTxPin<PIO, flexcomm::Usart4> for Pin<PIO, Special<function::FC4_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::UsartTxPin<PIO, flexcomm::Usart5> for Pin<PIO, Special<function::FC5_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::UsartTxPin<PIO, flexcomm::Usart6> for Pin<PIO, Special<function::FC6_TXD_SCL_MISO_WS>> {}
impl<PIO: PinId> fc::UsartTxPin<PIO, flexcomm::Usart7> for Pin<PIO, Special<function::FC7_TXD_SCL_MISO_WS>> {}
