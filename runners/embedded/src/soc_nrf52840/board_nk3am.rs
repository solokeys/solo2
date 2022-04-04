use nrf52840_pac::{
	Peripherals, CorePeripherals
};
use nrf52840_hal::{
	gpio::{p0, p1, Level, Pin, Output, PushPull},
	gpiote::Gpiote,
    prelude::{OutputPin, InputPin},
    pwm::Pwm,
    timer::Timer,
    pac, pwm, timer, spim,
};


pub const BOARD_NAME: &'static str = "NK3AM";
pub const KEEPALIVE_PINS: &'static [u8] = &[0x0b, 0x0c, 0x18, 0x19, 0x25, 0x26, 0x27, 0x28];

pub const USB_PRODUCT: &'static str = "Nitrokey NK3AM Meissn0rHack3d";
pub const USB_SERIAL: &'static str = "4bb17fc5-fddd-46f0-8244-cafecafecafe"; /* randomly generated */

pub const USB_ID_PRODUCT: u16 = 0x42ef_u16;


use crate::traits::rgb_led;
use crate::traits::rgb_led::Color;
use crate::traits::buttons::{
	Button, Press
};


pub type OutPin = Pin<Output<PushPull>>;


pub struct RgbLed {
	pwm_red: Pwm<pac::PWM0>,
	pwm_green: Pwm<pac::PWM1>,
	pwm_blue: Pwm<pac::PWM2>,
	timer_red: Timer<pac::TIMER1>,
	timer_green: Timer<pac::TIMER2>,
	timer_blue: Timer<pac::TIMER3>,
}

pub struct HardwareButtons {
    pub touch_button: Option<OutPin>,
}


use crate::soc::types::BoardGPIO;

use crate::soc::trussed_ui::UserInterface;

pub type TrussedUI = UserInterface<HardwareButtons, RgbLed>;



impl RgbLed {

    pub fn init_led<T: pwm::Instance, S: timer::Instance>(
        led: OutPin, 
        raw_pwm: T, 
		raw_timer: S,
        channel: pwm::Channel)
        -> (Pwm<T>, Timer<S>) {

		let pwm = Pwm::new(raw_pwm);
		pwm.set_output_pin(channel, led);
		
		//pwm.set_period(500u32.hz());
		//debug!("max duty: {:?}", pwm.max_duty());
		//pwm.set_max_duty(255);
		(pwm, Timer::new(raw_timer))
        
    }

    pub fn set_led(
        &mut self, 
        color: Color, 
        channel: pwm::Channel, 
        intensity: u8) {

            match color {
                Color::Red =>   {
                    let duty: u16 = ((intensity as f32 / 255.0) * self.pwm_red.max_duty() as f32) as u16;
                    self.pwm_red.set_duty_on(channel, duty as u16);
                },
                Color::Green => {
                    let duty: u16 = ((intensity as f32 / 255.0) * self.pwm_green.max_duty() as f32) as u16;
                    self.pwm_green.set_duty_on(channel, duty as u16);
                },
                Color::Blue =>  {
                    let duty: u16 = ((intensity as f32 / 255.0) * self.pwm_blue.max_duty() as f32) as u16;
                    self.pwm_blue.set_duty_on(channel, duty as u16);
                },
            }
        //}
    }
}

impl RgbLed {
    pub fn new (
        leds: [Option<OutPin>; 3], 
		pwm_red: pac::PWM0,
		timer_red: pac::TIMER1,
		pwm_green: pac::PWM1,
		timer_green: pac::TIMER2,
		pwm_blue: pac::PWM2,
		timer_blue: pac::TIMER3,
    ) -> RgbLed {

        let [mut red, mut green, mut blue] = leds;

        let (red_pwm_obj, red_timer_obj) = 
			RgbLed::init_led(red.unwrap(), pwm_red, timer_red, pwm::Channel::C0);
        let (green_pwm_obj, green_timer_obj) = 
			RgbLed::init_led(green.unwrap(), pwm_green, timer_green, pwm::Channel::C1);
        let (blue_pwm_obj, blue_timer_obj) = 
			RgbLed::init_led(blue.unwrap(), pwm_blue, timer_blue, pwm::Channel::C2);
        
        Self { 
            pwm_red: red_pwm_obj, pwm_green: green_pwm_obj, pwm_blue: blue_pwm_obj,
			timer_red: red_timer_obj, timer_green: green_timer_obj, timer_blue: blue_timer_obj

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




pub fn init_early(_device: &Peripherals, _core: &CorePeripherals) -> () {

}

pub fn init_ui(leds: [Option<OutPin>; 3], 
		pwm_red: pac::PWM0,
		timer_red: pac::TIMER1,
		pwm_green: pac::PWM1,
		timer_green: pac::TIMER2,
		pwm_blue: pac::PWM2,
		timer_blue: pac::TIMER3,
	 	touch: OutPin
	) -> TrussedUI {

	let rgb = RgbLed::new( 
		leds,
		pwm_red, timer_red,
		pwm_green, timer_green,
		pwm_blue, timer_blue,
	);

	let buttons = HardwareButtons {
		touch_button: Some(touch),
	};
	TrussedUI::new(Some(buttons), Some(rgb), true)
}



pub fn init_pins(gpiote: &Gpiote, gpio_p0: p0::Parts, gpio_p1: p1::Parts) -> BoardGPIO {
	
    /* touch sensor */
    let touch = gpio_p0.p0_04.into_push_pull_output(Level::High).degrade();
	// not used, just ensure output + low
	gpio_p0.p0_06.into_push_pull_output(Level::Low).degrade();
	
	/* irq configuration */

	// gpiote.port().input_pin(&btn3).low();
	// gpiote.port().input_pin(&btn4).low();
	// gpiote.port().input_pin(&btn5).low();
	// gpiote.port().input_pin(&btn6).low();
	// gpiote.port().input_pin(&btn7).low();
	// gpiote.port().input_pin(&btn8).low();

	/* RGB LED */
	let led_r = gpio_p0.p0_09.into_push_pull_output(Level::Low).degrade();
	let led_g = gpio_p0.p0_10.into_push_pull_output(Level::Low).degrade();
	let led_b = gpio_p1.p1_02.into_push_pull_output(Level::Low).degrade();
	
	/* SE050 */
	let se_pwr = gpio_p1.p1_10.into_push_pull_output(Level::Low).degrade();
	let se_scl = gpio_p1.p1_15.into_floating_input().degrade();
	let se_sda = gpio_p0.p0_02.into_floating_input().degrade();

	let se_pins = nrf52840_hal::twim::Pins {
		scl: se_scl,
		sda: se_sda
	};

	/* Ext. Flash SPI */
	// Flash WP# gpio_p0.p0_22
	// Flash HOLD# gpio_p0.p0_23
	let flash_spi_cs = gpio_p0.p0_24.into_push_pull_output(Level::High).degrade();
	let flash_spi_clk = gpio_p1.p1_06.into_push_pull_output(Level::Low).degrade();
	let flash_spi_mosi = gpio_p1.p1_04.into_push_pull_output(Level::Low).degrade();
	let flash_spi_miso = gpio_p1.p1_00.into_floating_input().degrade();
	//let _flash_wp = gpio_p0.p0_22.into_push_pull_output(Level::Low).degrade();
	//let _flash_hold = gpio_p0.p0_23.into_push_pull_output(Level::High).degrade();

	let flash_spi = spim::Pins {
		sck: flash_spi_clk,
		miso: Some(flash_spi_miso),
		mosi: Some(flash_spi_mosi)
	};

	BoardGPIO { buttons: [
			None, None, None, None, None, None, None, None ],
		leds: [ None, None, None, None ],
		rgb_led: [Some(led_r), Some(led_g), Some(led_b)],
		touch: Some(touch),
		uart_pins: None,
		fpr_detect: None,
		fpr_power: None,
		display_spi: None,
		display_cs: None,
		display_reset: None,
		display_dc: None,
		display_backlight: None,
		display_power: None,
		se_pins: Some(se_pins),
		se_power: Some(se_pwr),
		flashnfc_spi: Some(flash_spi),
		flash_cs: Some(flash_spi_cs),
		flash_power: None,
		nfc_cs: None,
		nfc_irq: None,
	}
}

pub fn gpio_irq_sources(dir: &[u32]) -> u32 {
	let mut src: u32 = 0;
	fn bit_set(x: u32, y: u32) -> bool { (x & (1u32 << y)) != 0 }

	if !bit_set(dir[0], 11) { src |= 0b0000_0001; }
	if !bit_set(dir[0], 12) { src |= 0b0000_0010; }
	if !bit_set(dir[0], 24) { src |= 0b0000_0100; }
	if !bit_set(dir[0], 25) { src |= 0b0000_1000; }
	if !bit_set(dir[1],  8) { src |= 0b0001_0000; }
	if !bit_set(dir[1],  7) { src |= 0b0010_0000; }
	if !bit_set(dir[1],  6) { src |= 0b0100_0000; }
	if !bit_set(dir[1],  5) { src |= 0b1000_0000; }
	src
}
