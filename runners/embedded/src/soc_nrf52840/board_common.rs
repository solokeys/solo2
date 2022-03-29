use nrf52840_hal::{
	gpio::{Pin, Output, PushPull, Level},
    prelude::{OutputPin, InputPin},
};

use core::convert::Infallible;




pub struct RgbLed {
    pub red: Option<Pin<Output<PushPull>>>,
    pub green: Option<Pin<Output<PushPull>>>,
    pub blue: Option<Pin<Output<PushPull>>>,
}

pub struct HardwareButtons {
    pub touch_button: Option<Pin<Output<PushPull>>>,
}

use super::traits::rgb_led;

use super::traits::buttons::{
	Button, Press
};



impl rgb_led::RgbLed for RgbLed {
    fn red(&mut self, intensity: u8){
        if let Some(led) = &mut self.red {
            if intensity > 127 {
                led.set_high();
            } else {
                led.set_low();
            }
        }
    }

    fn green(&mut self, intensity: u8){
        if let Some(led) = &mut self.green {
            if intensity > 127 {
                led.set_high();
            } else {
                led.set_low();
            }
        }        
    }

    fn blue(&mut self, intensity: u8) {
        if let Some(led) = &mut self.blue {
            if intensity > 127 {
                led.set_high();
            } else {
                led.set_low();
            }
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
