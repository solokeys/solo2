use nrf52840_hal::{
	gpio::{Pin, Output, PushPull, Level},
    prelude::{OutputPin, InputPin},
    pwm::Pwm,
    timer::Timer,
    pac, pwm, timer
};



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
use super::traits::rgb_led::Color;


use super::traits::buttons::{
	Button, Press
};



impl RgbLed {

    pub fn init_led<T: pwm::Instance, S: timer::Instance>(
        led: Option<OutPin>, 
        raw: Option<(T, S)>,
        channel: pwm::Channel)
        -> (Option<OutPin>, Option<(Pwm<T>, Timer<S>)>) {

        if led.is_some() && raw.is_some() {
            let (raw_pwm, raw_timer) = raw.unwrap();
            
            let pwm = Pwm::new(raw_pwm);
            pwm.set_output_pin(channel, led.unwrap());
            
            //pwm.set_period(500u32.hz());
            //debug!("max duty: {:?}", pwm.max_duty());
            //pwm.set_max_duty(255);
            (None, Some((pwm, Timer::new(raw_timer))))
        } else {
            (led, None)
        }
    }

    pub fn set_led(
        &mut self, 
        color: Color, 
        channel: pwm::Channel, 
        intensity: u8) {

        let mut cur_led = match color {
            Color::Red => self.red.as_mut(),
            Color::Green => self.green.as_mut(),
            Color::Blue => self.blue.as_mut(),
        };

        if let Some(led) = &mut cur_led {
            if intensity > 127 {
                led.set_high();
            } else {
                led.set_low();
            }
        } else {
            /* @TODO: this is sooooo wrong.... 1) rust! 2) rust arithmetic (rofl) 3) type-safety-no-friend */
            match color {
                Color::Red =>   {
                    let duty: u16 = ((intensity as f32 / 255.0) * self.pwm_red.as_ref().unwrap().0.max_duty() as f32) as u16;
                    self.pwm_red.as_ref().unwrap().0.set_duty_on(channel, duty as u16);
                },
                Color::Green => {
                    let duty: u16 = ((intensity as f32 / 255.0) * self.pwm_green.as_ref().unwrap().0.max_duty() as f32) as u16;
                    self.pwm_green.as_ref().unwrap().0.set_duty_on(channel, duty as u16);
                },
                Color::Blue =>  {
                    let duty: u16 = ((intensity as f32 / 255.0) * self.pwm_blue.as_ref().unwrap().0.max_duty() as f32) as u16;
                    self.pwm_blue.as_ref().unwrap().0.set_duty_on(channel, duty as u16);
                },
            }
            
            debug!("pwm set");
        }
    }
}

impl RgbLed {
    pub fn new (
        leds: [Option<Pin<Output<PushPull>>>; 3], 
        pwm_red: Option<(pac::PWM0, pac::TIMER1)>,
        pwm_green: Option<(pac::PWM1, pac::TIMER2)>,
        pwm_blue: Option<(pac::PWM2, pac::TIMER3)>,
    ) -> RgbLed {

        let [mut red, mut green, mut blue] = leds;

        let (red, red_pwm_obj) = RgbLed::init_led(red, pwm_red, pwm::Channel::C0);
        let (green, green_pwm_obj) = RgbLed::init_led(green, pwm_green, pwm::Channel::C1);
        let (blue, blue_pwm_obj) = RgbLed::init_led(blue, pwm_blue, pwm::Channel::C2);
        
        Self { 
            red, green, blue, 
            pwm_red: red_pwm_obj, pwm_green: green_pwm_obj, pwm_blue: blue_pwm_obj 
        }

    }
}

impl rgb_led::RgbLed for RgbLed {
    fn red(&mut self, intensity: u8){
        self.set_led(Color::Red, pwm::Channel::C0, intensity);
    }

    fn green(&mut self, intensity: u8){
        self.set_led(Color::Green, pwm::Channel::C1, intensity);
    }

    fn blue(&mut self, intensity: u8) {
        self.set_led(Color::Blue, pwm::Channel::C2, intensity);
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
