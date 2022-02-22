use crate::{
    peripherals::ctimer::Ctimer,
    time::Microseconds,
    traits::wg,
    typestates::init_state,
};

pub struct Pwm <TIMER>
where
    TIMER: Ctimer<init_state::Enabled>,
{
    timer: TIMER,
}

impl <TIMER> Pwm <TIMER>
where TIMER: Ctimer<init_state::Enabled> {

    pub fn new(timer: TIMER) -> Self {

        // Match should reset and stop timer, and generate interrupt.
        timer.mcr.modify(|_, w| {
            w
            .mr3i().set_bit()
            .mr3r().set_bit()
            .mr3s().clear_bit()
        });

        timer.pwmc.modify(|_,w|
            w.
            pwmen3().clear_bit()
        );

        // Set max duty cycle to 3rd match register (256 timer counts per pwm period)
        timer.mr[3].write(|w| unsafe { w.bits(0xff) });

        timer.mr[0].write(|w| unsafe{ w.bits(0x0) });
        timer.mr[1].write(|w| unsafe{ w.bits(0x0) });
        timer.mr[2].write(|w| unsafe{ w.bits(0x0) });

        timer.mcr.modify(|_,w| {
            w
            .mr0i().set_bit()
            .mr0r().clear_bit()
            .mr0s().clear_bit()

            .mr1i().set_bit()
            .mr1r().clear_bit()
            .mr1s().clear_bit()

            .mr2i().set_bit()
            .mr2r().clear_bit()
            .mr2s().clear_bit()
        });
        timer.pwmc.modify(|_,w|
            w
            .pwmen0().set_bit()
            .pwmen1().set_bit()
            .pwmen2().set_bit()
        );

        // No divsion necessary (1MHz / 256 ~= 4kHz at LED)
        timer.pr.write(|w| unsafe {w.bits(0)});

        // Start timer
        timer.tcr.write(|w| {
            w
            .crst().clear_bit()
            .cen().set_bit()
        });

        Self {
            timer: timer,
        }
    }

    pub fn release(self) -> TIMER {
        self.timer
    }

    /// Increase maximum value for the duty cycle.
    pub fn scale_max_duty_by(&mut self, duty: u32) {
        self.timer.mr[3].write(|w| unsafe { w.bits(0xff * duty) });
    }

}
//pin: & Pin<impl PinId, state::Analog<direction::Input>>

impl<TIMER> wg::Pwm for Pwm<TIMER>
where TIMER: Ctimer<init_state::Enabled>
{
    type Channel = u8;
    type Time = Microseconds;
    type Duty = u16;

    fn enable(&mut self, channel: Self::Channel) {
        match channel {
            0|1|2 => {

            }
            _ => {
                panic!("Cannot use channel outside 0-2 for PWM.");
            }
        }
    }

    fn disable(&mut self, channel: Self::Channel) {
        match channel {
            0 => {
                self.timer.mcr.modify(|_,w| {
                    w
                    .mr0i().clear_bit()
                    .mr0r().clear_bit()
                    .mr0s().clear_bit()
                });
                self.timer.pwmc.modify(|_,w|
                    w.
                    pwmen0().clear_bit()
                );
            }
            1 => {
                self.timer.mcr.modify(|_,w| {
                    w
                    .mr1i().clear_bit()
                    .mr1r().clear_bit()
                    .mr1s().clear_bit()
                });
                self.timer.pwmc.modify(|_,w|
                    w.
                    pwmen1().clear_bit()
                );
            }
            2 => {
                self.timer.mcr.modify(|_,w| {
                    w
                    .mr2i().clear_bit()
                    .mr2r().clear_bit()
                    .mr2s().clear_bit()
                });
                self.timer.pwmc.modify(|_,w|
                    w.
                    pwmen2().clear_bit()
                );
            }
            _ => {
                panic!("Cannot use channel outside 0-2 for PWM.");
            }
        }
    }

    fn get_period(&self) -> Self::Time {
        Microseconds(1_000_000 / self.get_max_duty() as u32)
    }

    fn set_period<P>(&mut self, _period: P)
    where
        P: Into<Self::Time>
    {
        panic!("Currently period is fixed.");
    }

    fn get_duty(&self, channel: Self::Channel) -> Self::Duty {
        self.timer.mr[channel as usize].read().bits() as Self::Duty
    }

    fn get_max_duty(&self) -> Self::Duty {
        self.timer.mr[3].read().bits() as Self::Duty
    }

    fn set_duty(&mut self, channel: Self::Channel, duty: Self::Duty) {
        self.timer.mr[channel as usize].write(|w| unsafe { w.bits(duty as u32) });
    }

}

