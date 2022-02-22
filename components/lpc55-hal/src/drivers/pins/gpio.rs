use crate::traits::{
    wg::{
        digital::v2::{
            InputPin,
            OutputPin,
            StatefulOutputPin,
            toggleable,
        },
    }
};

use crate::typestates::{
    pin::{
        state,
        gpio::{
            direction,
            Level,
        },
        PinId,
    },
    reg_proxy::RegClusterProxy,
};

use super::Pin;

use crate::{
    raw::gpio::{
        // B,
        // W,
        CLR,
        DIRSET,
        DIRCLR,
        PIN,
        SET,
    },
    reg_cluster,
};

// reg_cluster!(B, B, raw::GPIO, b);
// reg_cluster!(W, W, raw::GPIO, w);
reg_cluster!(DIRSET, DIRSET, raw::GPIO, dirset);
reg_cluster!(DIRCLR, DIRCLR, raw::GPIO, dirclr);
reg_cluster!(PIN, PIN, raw::GPIO, pin);
reg_cluster!(SET, SET, raw::GPIO, set);
reg_cluster!(CLR, CLR, raw::GPIO, clr);


impl<T> OutputPin for Pin<T, state::Gpio<direction::Output>>
where
    T: PinId,
{
    type Error = core::convert::Infallible;

    /// Set the pin output to HIGH
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.state.set[T::PORT].write(|w| unsafe { w.setp().bits(T::MASK) });
        Ok(())
    }

    /// Set the pin output to LOW
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.state.clr[T::PORT].write(|w| unsafe { w.clrp().bits(T::MASK) });
        Ok(())
    }
}

impl<T> StatefulOutputPin for Pin<T, state::Gpio<direction::Output>>
where
    T: PinId,
{
    fn is_set_high(&self) -> Result<bool, Self::Error> {
        Ok(self.state.pin[T::PORT].read().port().bits() & T::MASK == T::MASK)
    }

    fn is_set_low(&self) -> Result<bool, Self::Error> {
        Ok(!self.state.pin[T::PORT].read().port().bits() & T::MASK == T::MASK)
    }
}

impl<T: PinId> toggleable::Default for Pin<T, state::Gpio<direction::Output>> {}

impl<T> InputPin for Pin<T, state::Gpio<direction::Input>>
where
    T: PinId,
{
    type Error = core::convert::Infallible;

    fn is_high(&self) -> Result<bool, Self::Error> {
        // Ok(self.state.b[T::OFFSET].b_.read().pbyte())
        Ok(self.state.pin[T::PORT].read().port().bits() & T::MASK == T::MASK)
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        // Ok(!self.state.b.b_[T::OFFSET].read().pbyte())
        Ok(!self.state.pin[T::PORT].read().port().bits() & T::MASK == T::MASK)
    }
}

impl<T, D> Pin<T, state::Gpio<D>>
where
    T: PinId,
    D: direction::NotOutput,
{
    pub fn into_output_high(self) -> Pin<T, state::Gpio<direction::Output>> {
        self.into_output(Level::High)
    }
    pub fn into_output_low(self) -> Pin<T, state::Gpio<direction::Output>> {
        self.into_output(Level::Low)
    }
    pub fn into_output(self, initial: Level) -> Pin<T, state::Gpio<direction::Output>> {
        match initial {
            Level::High => self.state.set[T::PORT].write(|w| unsafe { w.setp().bits(T::MASK) }),
            Level::Low => self.state.clr[T::PORT].write(|w| unsafe { w.clrp().bits(T::MASK) }),
        }

        self.state.dirset[T::PORT].write(|w| unsafe { w.dirsetp().bits(T::MASK) });

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

                _direction: direction::Output,
            },
        }
    }
}

impl<T, D> Pin<T, state::Gpio<D>>
where
    T: PinId,
    D: direction::NotInput,
{
    pub fn into_input(self) -> Pin<T, state::Gpio<direction::Input>> {

        // currently, `into_gpio_pin()` sets `.digimode().digital()` in IOCON,
        // meaning input is enabled for all pins

        self.state.dirclr[T::PORT].write(|w| unsafe { w.dirclrp().bits(T::MASK) });

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

                _direction: direction::Input,
            },
        }
    }
}


impl<T, D> Pin<T, state::Analog<D>>
where
    T: PinId,
    D: direction::NotInput,
{
    pub fn into_input(self) -> Pin<T, state::Analog<direction::Input>> {

        // currently, `into_gpio_pin()` sets `.digimode().digital()` in IOCON,
        // meaning input is enabled for all pins

        self.state.dirclr[T::PORT].write(|w| unsafe { w.dirclrp().bits(T::MASK) });

        Pin {
            id: self.id,

            state: state::Analog {
                channel: self.state.channel,
                dirclr: RegClusterProxy::new(),
                _direction: direction::Input,
            },
        }
    }
}






