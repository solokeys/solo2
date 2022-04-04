use nrf52840_hal::{
	gpio::{p0, p1, Level},
	gpiote::Gpiote,
	spim,
};

use nrf52840_pac::{
	Peripherals, CorePeripherals
};

use crate::soc::types::BoardGPIO;



pub type TrussedUI = super::dummy_ui::DummyUI;

pub const BOARD_NAME: &'static str = "DK";
pub const KEEPALIVE_PINS: &'static [u8] = &[0x0b, 0x0c, 0x18, 0x19, 0x25, 0x26, 0x27, 0x28];

pub const USB_PRODUCT: &'static str = "Nitrokey NRFDK Eval";
pub const USB_SERIAL: &'static str = "4bb17fc5-fddd-46f0-8244-0da6bd13ca1b"; /* randomly generated */

pub const USB_ID_PRODUCT: u16 = 0x42ee_u16;


pub fn init_ui() -> TrussedUI {
	TrussedUI::new()
}

pub fn init_pins(gpiote: &Gpiote, gpio_p0: p0::Parts, gpio_p1: p1::Parts) -> BoardGPIO {
	/* Button 1-4: on DK */
	let btn1 = gpio_p0.p0_11.into_pullup_input().degrade();
	let btn2 = gpio_p0.p0_12.into_pullup_input().degrade();
	let btn3 = gpio_p0.p0_24.into_pullup_input().degrade();
	let btn4 = gpio_p0.p0_25.into_pullup_input().degrade();

	/* Button 5-8: wired through from Pico LCD */
	let btn5 = gpio_p1.p1_08.into_pullup_input().degrade();
	let btn6 = gpio_p1.p1_07.into_pullup_input().degrade();
	let btn7 = gpio_p1.p1_06.into_pullup_input().degrade();
	let btn8 = gpio_p1.p1_05.into_pullup_input().degrade();

	gpiote.port().input_pin(&btn1).low();
	gpiote.port().input_pin(&btn2).low();
	gpiote.port().input_pin(&btn3).low();
	gpiote.port().input_pin(&btn4).low();
	gpiote.port().input_pin(&btn5).low();
	gpiote.port().input_pin(&btn6).low();
	gpiote.port().input_pin(&btn7).low();
	gpiote.port().input_pin(&btn8).low();

	/* LEDs */
	let led1 = gpio_p0.p0_13.into_push_pull_output(Level::High).degrade();
	let led2 = gpio_p0.p0_14.into_push_pull_output(Level::High).degrade();
	let led3 = gpio_p0.p0_15.into_push_pull_output(Level::High).degrade();
	let led4 = gpio_p0.p0_16.into_push_pull_output(Level::High).degrade();

	/* UART */
	let u_rx = gpio_p0.p0_08.into_floating_input().degrade();
	let u_tx = gpio_p0.p0_06.into_push_pull_output(Level::High).degrade();

	let uart_pins = nrf52840_hal::uarte::Pins {
		txd: u_tx, rxd: u_rx, cts: None, rts: None
	};

	/* Display SPI Bus */
	let dsp_spi_dc = gpio_p1.p1_10.into_push_pull_output(Level::Low).degrade();
	let dsp_spi_cs = gpio_p1.p1_11.into_push_pull_output(Level::Low).degrade();
	let dsp_spi_clk = gpio_p1.p1_12.into_push_pull_output(Level::Low).degrade();
	let dsp_spi_mosi = gpio_p1.p1_13.into_push_pull_output(Level::Low).degrade();
	let dsp_spi_rst = gpio_p1.p1_14.into_push_pull_output(Level::Low).degrade();
	let dsp_spi_bl = gpio_p1.p1_15.into_push_pull_output(Level::High).degrade();
	// no power gate

	let dsp_spi = spim::Pins {
		sck: dsp_spi_clk,
		miso: None,
		mosi: Some(dsp_spi_mosi)
	};

	/* Ext. Flash SPI */
	// Flash WP# gpio_p0.p0_22
	// Flash HOLD# gpio_p0.p0_23
        let flash_spi_cs = gpio_p0.p0_17.into_push_pull_output(Level::High).degrade();
        let flashnfc_spi_clk = gpio_p0.p0_19.into_push_pull_output(Level::Low).degrade();
        let flashnfc_spi_mosi = gpio_p0.p0_20.into_push_pull_output(Level::Low).degrade();
        let flashnfc_spi_miso = gpio_p0.p0_21.into_floating_input().degrade();
	let _flash_wp = gpio_p0.p0_22.into_push_pull_output(Level::Low).degrade();
	let _flash_hold = gpio_p0.p0_23.into_push_pull_output(Level::High).degrade();

	let flashnfc_spi = spim::Pins {
		sck: flashnfc_spi_clk,
		miso: Some(flashnfc_spi_miso),
		mosi: Some(flashnfc_spi_mosi)
	};

	BoardGPIO { buttons: [
			Some(btn1), Some(btn2), Some(btn3), Some(btn4),
			Some(btn5), Some(btn6), Some(btn7), Some(btn8) ],
		leds: [ Some(led1), Some(led2), Some(led3), Some(led4) ],
		rgb_led: [None, None, None],
		touch: None,
		uart_pins: Some(uart_pins),
		fpr_detect: None,
		fpr_power: None,
		display_spi: Some(dsp_spi),
		display_cs: Some(dsp_spi_cs),
		display_reset: Some(dsp_spi_rst),
		display_dc: Some(dsp_spi_dc),
		display_backlight: Some(dsp_spi_bl),
		display_power: None,
		se_pins: None,
		se_power: None,
		flashnfc_spi: Some(flashnfc_spi),
		flash_cs: Some(flash_spi_cs),
		flash_power: None,
		nfc_cs: None,
		nfc_irq: None,
	}
}

/*
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
*/
