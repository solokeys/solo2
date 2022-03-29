use nrf52840_hal::{
	gpio::{p0, p1, Level},
	gpiote::Gpiote,
	spim,
};

use crate::soc::types::BoardGPIO;

pub const BOARD_NAME: &'static str = "Proto1";
pub const KEEPALIVE_PINS: &'static [u8] = &[0x29, 0x2b, 0x2d, 0x2f];

pub const USB_PRODUCT: &'static str = "Nitrokey/PTB Prototype #1";
pub const USB_SERIAL: &'static str = "493210be-43ea-4cc4-8d11-5bc82636c44f"; /* randomly generated */

pub const USB_ID_PRODUCT: u16 = 0x42ef_u16;

pub fn init_pins(gpiote: &Gpiote, gpio_p0: p0::Parts, gpio_p1: p1::Parts) -> BoardGPIO {
	/* Buttons */
	let btn1 = gpio_p1.p1_11.into_pullup_input().degrade();
	let btn2 = gpio_p1.p1_13.into_pullup_input().degrade();
	let btn3 = gpio_p1.p1_15.into_pullup_input().degrade();
	/* btn4 = p1_10 -- do not use, to be removed later */

	gpiote.port().input_pin(&btn1).low();
	gpiote.port().input_pin(&btn2).low();
	gpiote.port().input_pin(&btn3).low();

	/* Display SPI Bus */
	let dsp_spi_cs = gpio_p0.p0_06.into_push_pull_output(Level::Low).degrade();
	let dsp_spi_clk = gpio_p0.p0_01.into_push_pull_output(Level::Low).degrade();
	/* no MISO, unidirectional SPI */
	let dsp_spi_mosi = gpio_p0.p0_00.into_push_pull_output(Level::Low).degrade();
	let dsp_rst = gpio_p0.p0_04.into_push_pull_output(Level::Low).degrade();
	let dsp_dc = gpio_p0.p0_26.into_push_pull_output(Level::Low).degrade();
	let dsp_bl = gpio_p0.p0_08.into_push_pull_output(Level::High).degrade();
	let dsp_pwr = gpio_p0.p0_13.into_push_pull_output(Level::High).degrade();

	let dsp_spi = spim::Pins {
		sck: dsp_spi_clk,
		miso: None,
		mosi: Some(dsp_spi_mosi),
	};

	/* Fingerprint */
	let fp_tx = gpio_p0.p0_12.into_push_pull_output(Level::Low).degrade();
	let fp_rx = gpio_p0.p0_11.into_floating_input().degrade();
	let fp_detect = gpio_p1.p1_09.into_pulldown_input().degrade();
	let fp_pwr = gpio_p0.p0_15.into_push_pull_output(Level::High).degrade();

	let uart_pins = nrf52840_hal::uarte::Pins {
		txd: fp_tx, rxd: fp_rx, cts: None, rts: None
	};

	gpiote.port().input_pin(&fp_detect).high();

	/* SE050 */
	let se_pwr = gpio_p0.p0_20.into_push_pull_output(Level::Low).degrade();
	let se_scl = gpio_p0.p0_22.into_floating_input().degrade();
	let se_sda = gpio_p0.p0_24.into_floating_input().degrade();

	let se_pins = nrf52840_hal::twim::Pins {
		scl: se_scl,
		sda: se_sda
	};

	/* Flash & NFC SPI Bus */
	let flash_spi_cs = gpio_p0.p0_25.into_push_pull_output(Level::High).degrade();
	let nfc_spi_cs = gpio_p1.p1_01.into_push_pull_output(Level::High).degrade();
	let flashnfc_spi_clk = gpio_p1.p1_02.into_push_pull_output(Level::Low).degrade();
	let flashnfc_spi_miso = gpio_p1.p1_06.into_floating_input().degrade();
	let flashnfc_spi_mosi = gpio_p1.p1_04.into_push_pull_output(Level::Low).degrade();
	let flash_pwr = gpio_p1.p1_00.into_push_pull_output(Level::Low).degrade();
	let nfc_irq = gpio_p1.p1_07.into_pullup_input().degrade();

	let flashnfc_spi = spim::Pins {
		sck: flashnfc_spi_clk,
		miso: Some(flashnfc_spi_miso),
		mosi: Some(flashnfc_spi_mosi)
	};

	BoardGPIO { buttons: [
			Some(btn1), Some(btn2), Some(btn3), None,
			None, None, None, None ],
		leds: [ None, None, None, None ],
        rgb_led: [None, None, None], 
		touch: None,
		uart_pins: Some(uart_pins),
		fpr_detect: Some(fp_detect),
		fpr_power: Some(fp_pwr),
		display_spi: Some(dsp_spi),
		display_cs: Some(dsp_spi_cs),
		display_reset: Some(dsp_rst),
		display_dc: Some(dsp_dc),
		display_backlight: Some(dsp_bl),
		display_power: Some(dsp_pwr),
		se_pins: Some(se_pins),
		se_power: Some(se_pwr),
		flashnfc_spi: Some(flashnfc_spi),
		flash_cs: Some(flash_spi_cs),
		flash_power: Some(flash_pwr),
		nfc_cs: Some(nfc_spi_cs),
		nfc_irq: Some(nfc_irq),
	}
}

pub fn gpio_irq_sources(dir: &[u32]) -> u32 {
	let mut src: u32 = 0;
	fn bit_set(x: u32, y: u32) -> bool { (x & (1u32 << y)) != 0 }

	if !bit_set(dir[1], 11) { src |= 0b0000_0001; }
	if !bit_set(dir[1], 13) { src |= 0b0000_0010; }
	if !bit_set(dir[1], 15) { src |= 0b0000_0100; }
	if  bit_set(dir[1],  9) { src |= 0b1_0000_0000; }
	src
}
