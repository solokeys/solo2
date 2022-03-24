use nrf52840_pac::{
	Peripherals, CorePeripherals
};
use nrf52840_hal::{
	gpio::{p0, p1, Level},
	gpiote::Gpiote,
	spim,
};

use crate::soc::types::BoardGPIO;

pub const BOARD_NAME: &'static str = "NK3AM";
pub const KEEPALIVE_PINS: &'static [u8] = &[0x0b, 0x0c, 0x18, 0x19, 0x25, 0x26, 0x27, 0x28];

pub const USB_PRODUCT: &'static str = "Nitrokey NK3AM Meissn0rHack3d";
pub const USB_SERIAL: &'static str = "4bb17fc5-fddd-46f0-8244-cafecafecafe"; /* randomly generated */

pub const USB_ID_PRODUCT: u16 = 0x42ef_u16;


pub fn init_early(_device: &Peripherals, _core: &CorePeripherals) -> () {

}

pub fn init_pins(gpiote: &Gpiote, gpio_p0: p0::Parts, gpio_p1: p1::Parts) -> BoardGPIO {
	
    /* touch sensor */
    let touch = gpio_p0.p0_04.into_push_pull_output(Level::High).degrade();
	// not used, just ensure output + low
	gpio_p0.p0_06.into_push_pull_output(Level::Low).degrade();
	
	/* irq configuration */
	//gpiote.port().input_pin(&t_sense).low();
	//gpiote.port().input_pin(&t_rst).low();

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
		leds: [ Some(led_r), Some(led_g), Some(led_b), None ],
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

pub fn is_keepalive_pin(pinport: u32) -> bool {
	(pinport == 0x0b) ||
	(pinport == 0x0c) ||
	(pinport == 0x18) ||
	(pinport == 0x19) ||
	(pinport == 0x25) ||
	(pinport == 0x26) ||
	(pinport == 0x27) ||
	(pinport == 0x28)
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
