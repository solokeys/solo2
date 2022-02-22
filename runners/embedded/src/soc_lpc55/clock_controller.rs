use lpc55_hal::{
	prelude::*,
	Adc,
	Enabled,
	Gpio,
	Iocon,
	peripherals::adc::{self, ChannelType},
	Syscon,
	Pmc,
	drivers::{clocks::Clocks, pins::{self, Pin}},
	typestates::pin::{state, gpio:: direction},
};

pub type SignalPin = pins::Pio0_23;
pub type SignalButton = Pin<SignalPin, state::Gpio<direction::Output>>;

// pub type DynamicClockController = Adc<hal::typestates::init_state::Enabled>;
pub struct DynamicClockController {
    adc: lpc55_hal::raw::ADC0,
    #[allow(dead_code)]
    signal_button: SignalButton,
    clocks: Clocks,
    pmc: Pmc,
    syscon: Syscon,
    decrease_count: u32,
}

/// ADC measurement of internal 1V reference when VDD is approximately 2.35V
/// (14000 >> 3)/(2**12) * 2.35 == 1V
const ADC_VOLTAGE_LOW: u16 = 14_000;
/// ADC measurement of internal 1V reference when VDD is approximately 2.6V
/// (12700 >> 3)/(2**12) * 2.60 == 1V
const ADC_VOLTAGE_HIGH: u16 = 12_700;

impl DynamicClockController {
    pub fn adc_configuration() -> adc::Config {
        let mut config: adc::Config = Default::default();
        config.conversion_delay = 96;
        config
    }
    pub fn new(
        adc: Adc<lpc55_hal::typestates::init_state::Enabled>,
        clocks: Clocks,
        pmc: Pmc,
        syscon: Syscon,
        gpio: &mut Gpio<Enabled>,
        iocon: &mut Iocon<Enabled>,
    ) -> DynamicClockController {

        let signal_button = SignalPin::take().unwrap().into_gpio_pin(iocon, gpio).into_output_high();

        let adc = adc.release();

        adc.ie.write(|w| w.fwmie0().set_bit());

        adc.tctrl[ChannelType::Comparator as usize].write(|w| unsafe {
            w.hten().set_bit()
            .fifo_sel_a().fifo_sel_a_0()
            .fifo_sel_b().fifo_sel_b_0()
            .tcmd().bits(1)
            .tpri().bits(0)
            .tdly().bits(0)
        });

        adc.cmdl1.write(|w| unsafe {  w.adch().bits(13)     // 13 is internal 1v reference
                                    .ctype().ctype_0()
                                    .mode().mode_0()        // 12 bit resolution
                                    } );

        // shouldn't use more than 2^2 averages or compare seems to lock up
        adc.cmdh1.write(|w| unsafe { w.avgs().avgs_0()      // average 2^2 samples
                                    .cmpen().bits(0b11)        // compare repeatedly until true
                                    .loop_().bits(0)         // no loop
                                    .next().bits(0)         // no next command
                                    .sts().bits(2)
                                } );

        DynamicClockController {
            adc: adc,
            signal_button: signal_button,
            pmc: pmc,
            clocks: clocks,
            syscon: syscon,
            decrease_count: 0,
        }
    }

    pub fn start_low_voltage_compare(&mut self, ) {
        self.adc.cv1.write(|w| unsafe {
            w.cvl().bits(0)
            .cvh().bits(ADC_VOLTAGE_LOW)
        });

        self.adc.swtrig.write(|w| unsafe {w.bits(0)});
        self.adc.swtrig.write(|w| unsafe {w.bits(1<<(ChannelType::Comparator as usize))});
    }



    pub fn start_high_voltage_compare(&mut self, ) {
        self.adc.cv1.write(|w| unsafe {
            w.cvl().bits(ADC_VOLTAGE_HIGH)
            .cvh().bits(0x7ff8)
        });

        self.adc.swtrig.write(|w| unsafe {w.bits(0)});
        self.adc.swtrig.write(|w| unsafe {w.bits(1<<(ChannelType::Comparator as usize))});
    }

    fn decrease_clock(&mut self,){
        #[cfg(feature = "enable-clock-controller-signal-pin")]
        self.signal_button.set_low().ok();

        let requirements = lpc55_hal::ClockRequirements::default()
            .system_frequency(12.MHz());

        self.clocks = unsafe { requirements.reconfigure(self.clocks, &mut self.pmc, &mut self.syscon) };
        self.decrease_count += 1;
    }

    fn increase_clock(&mut self,){
        #[cfg(feature = "enable-clock-controller-signal-pin")]
        self.signal_button.set_high().ok();

        let requirements = if self.decrease_count > 2 {
            // opt for slower freq if there's too many dips in power
            lpc55_hal::ClockRequirements::default()
                .system_frequency(48.MHz())
        } else {
            lpc55_hal::ClockRequirements::default()
                .system_frequency(96.MHz())
        };

        self.clocks = unsafe { requirements.reconfigure(self.clocks, &mut self.pmc, &mut self.syscon) };
    }

    /// Used for debugging to tune the ADC points
    pub fn evaluate(&mut self){
        info_now!("status = {:02X}", self.adc.stat.read().bits());
        self.adc.cmdh1.modify(|_,w| unsafe { w
                                    .cmpen().bits(0)
                                } );
        for _ in 0 .. 50 {
            self.adc.swtrig.write(|w| unsafe {w.bits(0)});
            self.adc.swtrig.write(|w| unsafe {w.bits(1<<(ChannelType::Comparator as usize))});
            while self.adc.fctrl[0].read().fcount().bits() == 0 {
            }
            let result = self.adc.resfifo[0].read().bits();
            let _sample = (result & 0xffff) as u16;
            info_now!("Vref bias = {}", _sample);
        }
        self.adc.cmdh1.modify(|_,w| unsafe { w
                                    .cmpen().bits(0b11)
                                } );
    }

    pub fn handle(&mut self) {

        let count = self.adc.fctrl[0].read().fcount().bits();
        if count == 0 {
            info!("Error: no sample in fifo!");
            self.start_low_voltage_compare();
            return;
        }
        if count > 1 {
            info!("Got >1 sample!");
        }
        let result = self.adc.resfifo[0].read().bits();
        if  (result & 0x80000000) == 0 {
            panic!("underflow on compare");
        }
        let sample = (result & 0xffff) as u16;

        self.adc.ctrl.modify(|_,w| { w.rstfifo0().set_bit().rstfifo1().set_bit() });
        // info!("handle ADC: {}. status: {}", sample, self.adc.stat.read().bits());
        #[cfg(not(feature = "no-clock-controller"))]
        {

            if sample < ADC_VOLTAGE_HIGH {
                // info!("Voltage is high.  increase clock rate!");
                self.increase_clock();
                self.start_low_voltage_compare();
            } else if sample > ADC_VOLTAGE_LOW {
                // info!("Voltage is low.  Lower clock rate!");
                self.decrease_clock();
                self.start_high_voltage_compare();
            } else {
                // info!("Voltage is center: {}. Increase clock rate and watch closely!", sample);
                self.increase_clock();
                self.start_low_voltage_compare();
            }
        }
    }
}
