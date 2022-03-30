use nrf52840_hal::{
	gpio::{Pin, Output, PushPull, Level},
    prelude::{OutputPin, InputPin},
    pwm::Pwm,
    timer::Timer,
    pac, pwm
};


use core::convert::Infallible;

pub type OutPin = Pin<Output<PushPull>>;

pub struct RgbLed {
    pub red: Option<OutPin>,
    pub green: Option<OutPin>,
    pub blue: Option<OutPin>,
    
    pwm_red: Option<(Pwm<pac::PWM0>, Timer<pac::TIMER1>)>,
    pwm_green: Option<(Pwm<pac::PWM1>, Timer<pac::TIMER2>)>,
    pwm_blue: Option<(Pwm<pac::PWM2>, Timer<pac::TIMER3>)>,
}

pub struct HardwareButtons {
    pub touch_button: Option<OutPin>,
}

use super::traits::rgb_led;

use super::traits::buttons::{
	Button, Press
};


 /*

impl RgbLed {

    pub fn init_led<T, S>(led: Option<OutPin>, raw: Option<(T, S)>) -> (Option<OutPin>, Option<(Pwm<T>, Timer<S>)>) {
        if led.is_some() && raw.is_some() {
            let (raw_pwm, raw_timer) = raw.unwrap();
            let pwm = Pwm::new(raw_pwm);
            //
            if let Some(pin) = led {
                pwm.set_output_pin(pwm::Channel::C2, pin);
            };
            //pwm.set_period(500u32.hz());
            pwm.set_max_duty(255);
            (None, Some((pwm, Timer::new(raw_timer))))
        } else {
            (led, None)
        };
    }
}
*/

impl RgbLed {
    pub fn new (
        leds: [Option<Pin<Output<PushPull>>>; 3], 
        pwm_red: Option<(pac::PWM0, pac::TIMER1)>,
        pwm_green: Option<(pac::PWM1, pac::TIMER2)>,
        pwm_blue: Option<(pac::PWM2, pac::TIMER3)>,
    ) -> RgbLed {

        let [mut red, mut green, mut blue] = leds;

        // init red pwm if requested
        let (red, red_pwm_obj) = if red.is_some() && pwm_red.is_some() {
            let (raw_pwm, raw_timer) = pwm_red.unwrap();
            let pwm = Pwm::new(raw_pwm);
            //
            if let Some(pin) = red {
                pwm.set_output_pin(pwm::Channel::C0, pin);
            };
            //pwm.set_period(500u32.hz());
            pwm.set_max_duty(255);
            (None, Some((pwm, Timer::new(raw_timer))))
        } else {
            (red, None)
        };

        // init green pwm if requested
        let (green, green_pwm_obj) = if green.is_some() && pwm_green.is_some() {
            let (raw_pwm, raw_timer) = pwm_green.unwrap();
            let pwm = Pwm::new(raw_pwm);
            //
            if let Some(pin) = green {
                pwm.set_output_pin(pwm::Channel::C1, pin);
            };
            //pwm.set_period(500u32.hz());
            pwm.set_max_duty(255);
            (None, Some((pwm, Timer::new(raw_timer))))
        } else {
            (green, None)
        };

        // init blue pwm if requested
        let (blue, blue_pwm_obj) = if blue.is_some() && pwm_blue.is_some() {
            let (raw_pwm, raw_timer) = pwm_blue.unwrap();
            let pwm = Pwm::new(raw_pwm);
            //
            if let Some(pin) = blue {
                pwm.set_output_pin(pwm::Channel::C2, pin);
            };
            //pwm.set_period(500u32.hz());
            pwm.set_max_duty(255);
            (None, Some((pwm, Timer::new(raw_timer))))
        } else {
            (blue, None)
        };

        //let (blue, blue_pwm_obj) = RgbLed::init_led(blue, pwm_blue);
        

        Self { 
            red, green, blue, 
            pwm_red: red_pwm_obj, pwm_green: green_pwm_obj, pwm_blue: blue_pwm_obj 
        }

    }
}



impl rgb_led::RgbLed for RgbLed {
    fn red(&mut self, intensity: u8){
        if let Some(led) = &mut self.red {
            if intensity > 127 {
                led.set_high();
            } else {
                led.set_low();
            }
        } else {
            self.pwm_red.as_ref().unwrap().0.set_duty_on(pwm::Channel::C0, intensity as u16);
            debug!("pwm set");
        }
    }

    fn green(&mut self, intensity: u8){
        if let Some(led) = &mut self.green {
            if intensity > 127 {
                led.set_high();
            } else {
                led.set_low();
            }
        } else {
            self.pwm_green.as_ref().unwrap().0.set_duty_on(pwm::Channel::C1, intensity as u16);
        }       
    }

    fn blue(&mut self, intensity: u8) {
        if let Some(led) = &mut self.blue {
            if intensity > 127 {
                led.set_high();
            } else {
                led.set_low();
            }
        } else {
            self.pwm_blue.as_ref().unwrap().0.set_duty_on(pwm::Channel::C2, intensity as u16);
        }
    }
}

impl Press for HardwareButtons {
	fn is_pressed(&mut self, but: Button) -> bool {
        match but {
            
            Button::A => {
                let mut ticks = 0;
		
                if let Some(touch) = self.touch_button.take() {
                    let floating = touch.into_floating_input();

                    for idx in 0..10_000 {
                        match floating.is_low() {
                            Err(e) => { debug!("err!"); },
                            Ok(v) => match v {
                                true => { 
                                    ticks = idx; 
                                    break; 
                                },
                                false => { }
                            },
                        }
                    }
                    self.touch_button = Some(floating.into_push_pull_output(Level::High));
                }
                ticks > 50
            }
            _ => {
                false
            }
        }		
	}
}
