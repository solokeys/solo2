use core::ops::Deref;
use crate::{
    raw,
    peripherals::{
        syscon::Syscon,
        pmc::Pmc,
    },
    drivers::{
        pins::Pin,
    },
    typestates::{
        init_state,
        pin::{
            state,
            gpio::{
                direction,
            },
            PinId,
        },
    }
};

pub struct Config {
    pub conversion_delay: u16,
}

impl Default for Config {
    fn default() -> Self {
        Config{
            conversion_delay: 0,
        }
    }
}

pub enum ChannelType {
    Comparator = 0,
    Cancel = 1,
    Normal = 2,
}

pub struct Adc<State = init_state::Unknown> {
    pub(crate) raw: raw::ADC0,
    pub _state: State,
    config: Config,
}

impl core::convert::From<raw::ADC0> for Adc {
    fn from(raw: raw::ADC0) -> Self {
        Adc::new(raw)
    }
}

impl Adc {
    fn new(raw: raw::ADC0) -> Self {
        Adc {
            raw,
            _state: init_state::Unknown,
            config: Default::default(),
        }
    }

    pub unsafe fn steal() -> Self {
        // seems a little wastefule to steal the full peripherals but ok..
        Self::new(raw::Peripherals::steal().ADC0)
    }
}

impl<State> Adc<State> {
    pub fn release(self) -> raw::ADC0 {
        self.raw
    }
}

impl<State> Deref for Adc<State> {
    type Target = raw::adc0::RegisterBlock;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<State> Adc<State> {
    #[allow(dead_code)]
    fn autocal (&mut self,) {
        // Calibration + offset trimming
        self.raw.ofstrim.write(|w| unsafe {
            w.ofstrim_a().bits(10)
            .ofstrim_b().bits(10)
        });

        // Request calibration
        self.raw.ctrl.modify(|_,w| {w.cal_req().set_bit()});

        // wait for auto-cal to be ready.
        while (!self.raw.gcc[0].read().rdy().bits()) || (!self.raw.gcc[1].read().rdy().bits()) {
        }

        let gain_a = self.raw.gcc[0].read().gain_cal().bits();
        let gain_b = self.raw.gcc[1].read().gain_cal().bits();

        let gcr_a = (((gain_a as u32) << 16) / (0x1FFFFu32 - gain_a as u32 )) as u16;
        let gcr_b = (((gain_b as u32) << 16) / (0x1FFFFu32 - gain_b as u32 )) as u16;

        self.raw.gcr[0].write(|w| unsafe {w.gcalr().bits(gcr_a)});
        self.raw.gcr[1].write(|w| unsafe {w.gcalr().bits(gcr_b)});

        self.raw.gcr[0].write(|w| {w.rdy().set_bit()});
        self.raw.gcr[1].write(|w| {w.rdy().set_bit()});

        while !self.raw.stat.read().cal_rdy().bits() {
        }
    }

    pub fn arm_normal_channel(&mut self, channel_id: u8) {
        self.raw.cmdl2.write(|w| unsafe {  w.adch().bits(channel_id)
                                    .ctype().ctype_0()
                                    .mode().mode_0()
                                    } );
        self.raw.cmdh2.write(|w| unsafe { w.avgs().avgs_7()
                                    .cmpen().bits(0b00)        // no compare
                                    .loop_().bits(0)
                                    .next().bits(0)
                                } );
    }

    pub fn arm_comparator_channel(&mut self, channel_id: u8) {
        self.raw.cmdl1.write(|w| unsafe {  w.adch().bits(channel_id)
                                    .ctype().ctype_0()
                                    .mode().mode_0()
                                    } );
        self.raw.cmdh1.write(|w| unsafe { w.avgs().avgs_7()      // average 2^3 samples
                                    .cmpen().bits(0b11)        // compare repeatedly until true
                                    .loop_().bits(0)         // no loop
                                    .next().bits(0)         // no next command
                                } );
    }

    pub fn set_threshold(&mut self, low: u16, high: u16) {
        self.raw.cv1.write(|w| unsafe {
            w.cvl().bits(low)
            .cvh().bits(high)
        })
    }

    pub fn cancel_compare(&mut self){
        self.raw.swtrig.write(|w| unsafe {w.bits(2)});      // Trigger the cancel trigger
        self.raw.ctrl.modify(|_,w| { w.rstfifo0().set_bit().rstfifo1().set_bit() })
    }

    pub fn configure(mut self, config: Config) -> Adc<State> {
        self.config = config;
        self
    }

    pub fn enabled(mut self, pmc: &mut Pmc, syscon: &mut Syscon) -> Adc<init_state::Enabled> {
        syscon.enable_clock(&mut self.raw);
        syscon.reset(&mut self.raw);
        syscon.raw.adcclkdiv.write(|w| {w.reset().set_bit()});
        syscon.raw.adcclkdiv.write(|w| unsafe {w.div().bits(0)});
        syscon.raw.adcclkdiv.write(|w| unsafe {w.bits(0)});

        syscon.raw.adcclksel.write(|w| {w.sel().fro96()});

        pmc.power_on(&mut self.raw);

        self.raw.ctrl.write(|w| {w.rst().set_bit()});   // Reset
        self.raw.ctrl.write(|w| {w.rst().clear_bit()});

        self.raw.ctrl.write(|w| {w.rstfifo0().set_bit()});  // Reset FIFO
        self.raw.ctrl.write(|w| {w.rstfifo0().clear_bit()});

        self.raw.ctrl.write(|w| {w.rstfifo1().set_bit()});  // Reset FIFO
        self.raw.ctrl.write(|w| {w.rstfifo1().clear_bit()});

        self.raw.ctrl.write(|w| {
            w.adcen().clear_bit()  // Turn off prior to configuration
        });
        self.raw.cfg.write(|w| {
            w.pwren().clear_bit()  //Must be cleared prior to ADC being enabled
        });


        self.raw.ctrl.write(|w| {
            w.dozen().set_bit()
            .cal_avgs().cal_avgs_7()
        });

        self.raw.cfg.write(|w| unsafe {
            w.pwren().set_bit()
            .pudly().bits(0x80)
            .refsel().refsel_1()
            .pwrsel().pwrsel_3()
            .tprictrl().bits(0)
            .tres().clear_bit() // Do not resume interrupted captures
        });


        if self.config.conversion_delay > 0 {
            self.raw.pause.write(|w| unsafe {
                w.pausedly().bits(self.config.conversion_delay)
                .pauseen().set_bit()
            });
        } else {
            self.raw.pause.write(|w| unsafe {
                w.pausedly().bits(0)
                .pauseen().clear_bit()
            });
        }

        // Set 0 for watermark
        self.raw.fctrl[0].write(|w| unsafe{w.fwmark().bits(0)});
        self.raw.fctrl[1].write(|w| unsafe{w.fwmark().bits(0)});


        // turn on!
        self.raw.ctrl.modify(|_, w| {w.adcen().set_bit()});

        // This breaks adc when it's not being run from debugger for some reason
        // self.autocal();

        self.arm_comparator_channel(3);
        self.arm_normal_channel(3);

        // Main trigger
        self.raw.tctrl[ChannelType::Comparator as usize].write(|w| unsafe {
            w.hten().set_bit()
            .fifo_sel_a().fifo_sel_a_0()
            .fifo_sel_b().fifo_sel_b_0()
            .tcmd().bits(1)
            .tpri().bits(3)
        });

        // Cancel/resync trigger to main trigger
        self.raw.tctrl[ChannelType::Cancel as usize].write(|w| unsafe {
            w.hten().set_bit()
            .fifo_sel_a().fifo_sel_a_0()
            .fifo_sel_b().fifo_sel_b_0()
            .tcmd().bits(0)
            .rsync().set_bit()
            .tpri().bits(0) //highest priority
        });

        // Normal measurement trigger
        self.raw.tctrl[ChannelType::Normal as usize].write(|w| unsafe {
            w.hten().set_bit()
            .fifo_sel_a().fifo_sel_a_0()
            .fifo_sel_b().fifo_sel_b_0()
            .tcmd().bits(2)
            .tpri().bits(2)
        });


        Adc {
            raw: self.raw,
            _state: init_state::Enabled(()),
            config: Default::default(),
        }
    }

    pub fn disabled(mut self, syscon: &mut Syscon) -> Adc<init_state::Disabled> {
        self.raw.ctrl.write(|w| {w.adcen().clear_bit()});
        syscon.raw.adcclksel.write(|w| {w.sel().none()});
        syscon.disable_clock(&mut self.raw);

        Adc {
            raw: self.raw,
            _state: init_state::Disabled,
            config: Default::default(),
        }
    }
}

// use crate::traits::wg::adc;

type Result<T> = core::result::Result<T, Underflow>;

#[derive(Debug, Clone)]
pub struct Underflow;

impl<State> Adc <State>
{
    // type Error = Underflow;

    // Read normal sample
    pub fn read(&mut self, pin: & Pin<impl PinId, state::Analog<direction::Input>>) -> Result<u16> {
        self.arm_normal_channel(pin.state.channel);

        self.raw.swtrig.write(|w| unsafe {w.bits(1<<(ChannelType::Normal as usize))});
        while self.raw.fctrl[0].read().fcount().bits() == 0 {
        }
        let result = self.raw.resfifo[0].read().bits();
        if  (result & 0x80000000) == 0 {
            return Err(Underflow);
        }
        let sample = (result & 0xffff) as u16;
        Ok(sample)
    }
}
