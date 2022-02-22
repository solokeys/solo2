#![no_std]

//! This HAL takes a layered approach.
//!
//! 1. raw PAC peripherals
//! 1. HAL peripheral wrappers (under `peripherals`)
//! 1. HAL drivers (under `drivers`, typically take ownership of one or more peripherals)
//!
//! The middle layer is quite thin, notably we model pins and the clock tree
//! as drivers.
//!
//! In as much as possible, it is a goal for this HAL that drivers implement
//! general interfaces (under `traits`).
//!
//! The main intended use case of this HAL is in the context of RTIC.
//!
//! To get started without RTIC, try something like:
//! ```
//! let hal = hal::Peripherals::take().unwrap(); // layer 2
//! let pins = hal::Pins::take().unwrap(); // layer 3
//!
//! let mut syscon = hal.syscon;
//! let mut gpio = hal.gpio.enabled(&mut syscon);
//! let mut iocon = hal.iocon.enabled(&mut syscon);
//!
//! let mut red_led = pins.pio1_6
//!     .into_gpio_pin(&mut iocon, &mut gpio)
//!     .into_output(Level::High);
//!
//! loop {
//!     red.set_low().unwrap();
//!     hal::wait_at_least(300_000);
//!     red.set_high().unwrap();
//!     hal::wait_at_least(300_000);
//! }
//! ```

pub extern crate lpc55_pac as raw;

pub mod prelude;

// #[macro_use]
pub mod macros;

pub mod time;
pub mod traits;

pub mod typestates;
pub use typestates::{
    init_state::Enabled,
};

pub mod peripherals;
pub use peripherals::{
    adc::Adc,
    anactrl::Anactrl,
    casper::Casper,
    ctimer::Ctimers,
    dma::Dma,
    flash::Flash,
    flexcomm::Flexcomm,
    gpio::Gpio,
    gint::Gint,
    hashcrypt::Hashcrypt,
    inputmux::InputMux,
    iocon::Iocon,
    pint::Pint,
    pfr::Pfr,
    pmc::Pmc,
    prince::Prince,
    puf::Puf,
    rng::Rng,
    rtc::Rtc,
    syscon::Syscon,
    usbfs::Usbfs,
    usbhs::Usbhs,
    utick::Utick,
};

pub mod drivers;
pub use drivers::{
    ClockRequirements,
    FlashGordon,
    I2cMaster,
    SpiMaster,
    Pin,
    Pins,
    UsbBus,
};


pub fn new() -> Peripherals {
    take().unwrap()
}

/// This is the main (monolithic) entry point to the HAL for non-RTIC applications.
/// For RTIC, use `hal::<Peripheral>::from(<raw_peripheral>)` as needed.
pub fn take() -> Option<Peripherals> {
    Some(Peripherals::from((
        raw::Peripherals::take()?,//.expect("raw device peripherals already taken elsewhere"),
        raw::CorePeripherals::take()?,//.expect("raw core peripherals already taken elsewhere"),
    )))
}

#[cfg(not(feature = "rtic-peripherals"))]
pub fn from(raw: (raw::Peripherals, raw::CorePeripherals)) -> Peripherals {
    Peripherals::from(raw)
}

#[cfg(feature = "rtic-peripherals")]
pub fn from(raw: (raw::Peripherals, rtic::Peripherals)) -> Peripherals {
    Peripherals::from(raw)
}

/// This is the entry point to the HAL API.
///
/// Before you can do anything else, you need to get an instance of this struct,
/// via `hal::new` or `hal::steal`.
#[allow(non_snake_case)]
pub struct Peripherals {

    /// Analog-to-Digital Converter (ADC)
    pub adc: Adc,

    /// Analog control
    pub anactrl: Anactrl,

    /// Cryptographic Accelerator and Signal Processing Engine with RAM sharing
    pub casper: Casper,

    /// Standard counter/timer (CTIMER)
    pub ctimer: Ctimers,

    /// Direct memory access
    pub dma: Dma,

    /// Flash
    pub flash: Flash,

    /// Flexcomm Interface Serial Communication
    pub flexcomm: Flexcomm,

    /// Group GPIO Input Interrupt
    pub gint: Gint,

    /// General-purpose I/O (GPIO)
    pub gpio: Gpio,

    /// SHA and AES Engine
    pub hashcrypt: Hashcrypt,

    /// Input multiplexer
    pub inputmux: InputMux,

    /// I/O configuration
    pub iocon: Iocon,

    /// Pin Interrupt and Pattern Match
    pub pint: Pint,

    /// Protect flash region controller
    pub pfr: Pfr,

    /// Power configuration
    pub pmc: Pmc,

    // PRINCE
    pub prince: Prince,

    /// Random number generator
    pub rng: Rng,

    /// Real time clock
    pub rtc: Rtc,

    /// System configuration
    pub syscon: Syscon,

    /// USB full-speed device or, not implemented, host
    pub usbfs: Usbfs,

    /// USB high-speed device or, not implemented, host
    pub usbhs: Usbhs,

    /// Micro-Tick Timer
    pub utick: Utick,


    /// CRC engine - not HAL-ified.
    pub CRC_ENGINE: raw::CRC_ENGINE,

    pub FLASH_CMPA: raw::FLASH_CMPA,
    pub FLASH_CFPA0: raw::FLASH_CFPA0,

    /// Stateful counter/timer (SCTIMER) - not HAL-ified.
    pub SCT0: raw::SCT0,

    /// SAU - not HAL-ified.
    pub SAU: raw::SAU,

    /// AHB_SECURE_CTRL - not HAL-ified.
    pub AHB_SECURE_CTRL: raw::AHB_SECURE_CTRL,

    /// CPUID - core peripheral
    pub CPUID: raw::CPUID,

    /// Debug Control Block (DCB) - core peripheral
    pub DCB: raw::DCB,

    /// Data Watchpoint and Trace unit (DWT) - core peripheral
    pub DWT: raw::DWT,

    /// Memory Protection Unit (MPU) - core peripheral
    pub MPU: raw::MPU,

    /// Nested Vector Interrupt Controller (NVIC) - core peripheral
    pub NVIC: raw::NVIC,

    /// System Control Block (SCB) - core peripheral
    pub SCB: raw::SCB,

    #[cfg(not(feature = "rtic-peripherals"))]
    /// SysTick: System Timer - core peripheral
    #[cfg(not(feature = "rtic-peripherals"))]
    pub SYST: raw::SYST,
}

#[cfg(feature = "rtic-peripherals")]
impl From<(raw::Peripherals, rtic::Peripherals)> for Peripherals {
    fn from(raw: (raw::Peripherals, rtic::Peripherals)) -> Self {
        let cp = raw.1;
        let p = raw.0;
        Peripherals {
            // HAL peripherals
            adc: Adc::from(p.ADC0),
            anactrl: Anactrl::from(p.ANACTRL),
            casper: Casper::from(p.CASPER),
            ctimer: (
                peripherals::ctimer::Ctimer0::from(p.CTIMER0),
                peripherals::ctimer::Ctimer1::from(p.CTIMER1),
                peripherals::ctimer::Ctimer2::from(p.CTIMER2),
                peripherals::ctimer::Ctimer3::from(p.CTIMER3),
                peripherals::ctimer::Ctimer4::from(p.CTIMER4),
            ),
            dma: Dma::from(p.DMA0),
            flash: Flash::from(p.FLASH),
            flexcomm: (
                peripherals::flexcomm::Flexcomm0::from((p.FLEXCOMM0, p.I2C0, p.I2S0, p.SPI0, p.USART0)),
                peripherals::flexcomm::Flexcomm1::from((p.FLEXCOMM1, p.I2C1, p.I2S1, p.SPI1, p.USART1)),
                peripherals::flexcomm::Flexcomm2::from((p.FLEXCOMM2, p.I2C2, p.I2S2, p.SPI2, p.USART2)),
                peripherals::flexcomm::Flexcomm3::from((p.FLEXCOMM3, p.I2C3, p.I2S3, p.SPI3, p.USART3)),
                peripherals::flexcomm::Flexcomm4::from((p.FLEXCOMM4, p.I2C4, p.I2S4, p.SPI4, p.USART4)),
                peripherals::flexcomm::Flexcomm5::from((p.FLEXCOMM5, p.I2C5, p.I2S5, p.SPI5, p.USART5)),
                peripherals::flexcomm::Flexcomm6::from((p.FLEXCOMM6, p.I2C6, p.I2S6, p.SPI6, p.USART6)),
                peripherals::flexcomm::Flexcomm7::from((p.FLEXCOMM7, p.I2C7, p.I2S7, p.SPI7, p.USART7)),
                peripherals::flexcomm::Flexcomm8::from((p.FLEXCOMM8, p.SPI8)),
            ),
            gint: Gint::from((p.GINT0, p.GINT1)),
            gpio: Gpio::from(p.GPIO),
            hashcrypt: Hashcrypt::from(p.HASHCRYPT),
            inputmux: InputMux::from(p.INPUTMUX),
            iocon: Iocon::from(p.IOCON),
            pint: Pint::from(p.PINT),
            pfr: Pfr::new(),
            pmc: Pmc::from(p.PMC),
            prince: Prince::from(p.PRINCE),
            rng: Rng::from(p.RNG),
            rtc: Rtc::from(p.RTC),
            syscon: Syscon::from(p.SYSCON),
            usbfs: Usbfs::from((p.USB0, p.USBFSH)),
            usbhs: Usbhs::from((p.USBPHY, p.USB1, p.USBHSH)),
            utick: Utick::from(p.UTICK0),

            // Raw peripherals
            AHB_SECURE_CTRL: p.AHB_SECURE_CTRL,
            CRC_ENGINE: p.CRC_ENGINE,
            FLASH_CMPA: p.FLASH_CMPA,
            FLASH_CFPA0: p.FLASH_CFPA0,
            SAU: p.SAU,
            SCT0: p.SCT0,

            // Core peripherals
            CPUID: cp.CPUID,
            DCB: cp.DCB,
            DWT: cp.DWT,
            MPU: cp.MPU,
            NVIC: cp.NVIC,
            SCB: cp.SCB,
        }
    }
}

impl From<(raw::Peripherals, raw::CorePeripherals)> for Peripherals {
    fn from(raw: (raw::Peripherals, raw::CorePeripherals)) -> Self {
        let cp = raw.1;
        let p = raw.0;
        Peripherals {
            // HAL peripherals
            adc: Adc::from(p.ADC0),
            anactrl: Anactrl::from(p.ANACTRL),
            casper: Casper::from(p.CASPER),

            ctimer: (
                peripherals::ctimer::Ctimer0::from(p.CTIMER0),
                peripherals::ctimer::Ctimer1::from(p.CTIMER1),
                peripherals::ctimer::Ctimer2::from(p.CTIMER2),
                peripherals::ctimer::Ctimer3::from(p.CTIMER3),
                peripherals::ctimer::Ctimer4::from(p.CTIMER4),
            ),
            dma: Dma::from(p.DMA0),
            flash: Flash::from(p.FLASH),
            flexcomm: (
                peripherals::flexcomm::Flexcomm0::from((p.FLEXCOMM0, p.I2C0, p.I2S0, p.SPI0, p.USART0)),
                peripherals::flexcomm::Flexcomm1::from((p.FLEXCOMM1, p.I2C1, p.I2S1, p.SPI1, p.USART1)),
                peripherals::flexcomm::Flexcomm2::from((p.FLEXCOMM2, p.I2C2, p.I2S2, p.SPI2, p.USART2)),
                peripherals::flexcomm::Flexcomm3::from((p.FLEXCOMM3, p.I2C3, p.I2S3, p.SPI3, p.USART3)),
                peripherals::flexcomm::Flexcomm4::from((p.FLEXCOMM4, p.I2C4, p.I2S4, p.SPI4, p.USART4)),
                peripherals::flexcomm::Flexcomm5::from((p.FLEXCOMM5, p.I2C5, p.I2S5, p.SPI5, p.USART5)),
                peripherals::flexcomm::Flexcomm6::from((p.FLEXCOMM6, p.I2C6, p.I2S6, p.SPI6, p.USART6)),
                peripherals::flexcomm::Flexcomm7::from((p.FLEXCOMM7, p.I2C7, p.I2S7, p.SPI7, p.USART7)),
                peripherals::flexcomm::Flexcomm8::from((p.FLEXCOMM8, p.SPI8)),
            ),
            gint: Gint::from((p.GINT0, p.GINT1)),
            gpio: Gpio::from(p.GPIO),
            hashcrypt: Hashcrypt::from(p.HASHCRYPT),
            inputmux: InputMux::from(p.INPUTMUX),
            iocon: Iocon::from(p.IOCON),
            pint: Pint::from(p.PINT),
            pfr: Pfr::new(),
            pmc: Pmc::from(p.PMC),
            prince: Prince::from(p.PRINCE),
            rng: Rng::from(p.RNG),
            rtc: Rtc::from(p.RTC),
            syscon: Syscon::from(p.SYSCON),
            usbfs: Usbfs::from((p.USB0, p.USBFSH)),
            usbhs: Usbhs::from((p.USBPHY, p.USB1, p.USBHSH)),
            utick: Utick::from(p.UTICK0),

            // Raw peripherals
            AHB_SECURE_CTRL: p.AHB_SECURE_CTRL,
            CRC_ENGINE: p.CRC_ENGINE,
            FLASH_CMPA: p.FLASH_CMPA,
            FLASH_CFPA0: p.FLASH_CFPA0,
            SAU: p.SAU,
            SCT0: p.SCT0,

            // Core peripherals
            CPUID: cp.CPUID,
            DCB: cp.DCB,
            DWT: cp.DWT,
            MPU: cp.MPU,
            NVIC: cp.NVIC,
            SCB: cp.SCB,
            #[cfg(not(feature = "rtic-peripherals"))]
            SYST: cp.SYST,
        }
    }
}

impl Peripherals {

    #[cfg(not(feature = "rtic-peripherals"))]
    pub fn take() -> Option<Self> {
        Some(Self::from((
            raw::Peripherals::take()?,
            raw::CorePeripherals::take()?,
        )))
    }

    // rtic::Peripherals::take does not exist
    //
    // #[cfg(feature = "rtic-peripherals")]
    // pub fn take() -> Option<Self> {
    //     Some(Self::from((
    //         raw::Peripherals::take()?,
    //         rtic::Peripherals::take()?,
    //     )))
    // }

    #[cfg(not(feature = "rtic-peripherals"))]
    pub unsafe fn steal() -> Self {
        Self::from((raw::Peripherals::steal(), raw::CorePeripherals::steal()))
    }

}

pub fn enable_cycle_counter() {
    unsafe { &mut raw::CorePeripherals::steal().DWT }.enable_cycle_counter();
}

pub fn get_cycle_count() -> u32 {
    raw::DWT::cycle_count()
}

pub fn count_cycles<Output>(f: impl FnOnce() -> Output) -> (u32, Output) {
    let before = get_cycle_count();
    let outcome = f();
    let after = get_cycle_count();
    (after - before, outcome)
}


/// Delay of last resort :-))
pub fn wait_at_least(delay_usecs: u32) {
    enable_cycle_counter();
    let max_cpu_speed = 150_000_000; // via PLL
    let period = max_cpu_speed / 1_000_000;

    let current = get_cycle_count() as u64;
    let mut target = current + period as u64 * delay_usecs as u64;
    if target > 0xFFFF_FFFF {
        // wait for wraparound
        target -= 0xFFFF_FFFF;
        while target < get_cycle_count() as u64 { continue; }
    }
    while target > get_cycle_count() as u64 { continue; }
}

/// https://community.nxp.com/t5/LPC-Microcontrollers-Knowledge/Understanding-LPC55S6x-Revisions-and-Tools/ta-p/1117604
///
/// Note that: EVK A1 = chip 0A, EVK A2 = chip 1B
pub fn chip_revision() -> &'static str {
    const DIEID: *const u8 = 0x5000_0FFC as _;
    let rev_id: u8 = 0xFu8 & unsafe { core::ptr::read_volatile(DIEID) };
    match rev_id {
        0 => "0A",
        1 => "1B",
        _ => "unknown",
    }

}

pub fn uuid() -> [u8; 16] {
    const UUID: *const u8 = 0x0009_FC70 as _;
    let mut uuid: [u8; 16] = [0; 16];
    for i in 0..16 {
        uuid[i] = unsafe { *UUID.offset(i as isize) };
    }
    uuid
}

/// This is a hack to jump to the bootrom without needing to assert ISP pin
/// or destroy current firmware.
///
/// 1. Resets all peripherals & disconnect all interrupts (like in a soft reset)
/// 2. Enable Iocon and set the INVERT attribute for Pio0_5 (ISP pin).
/// 3. Jump to bootrom, which will think ISP pin is asserted.
///
/// Other prerequisites for this to work:
/// - Must not be called from an interrupt handler.
/// - TrustZone must not be disabled (unless you can find a way to re-enable it here).
pub fn boot_to_bootrom() -> ! {


    // Disconnect all interrupts
    cortex_m::interrupt::disable();
    let core_peripherals = unsafe { cortex_m::peripheral::Peripherals::steal() };
    unsafe {
        core_peripherals.NVIC.icer[0].write(0xFFFF_FFFFu32);
        core_peripherals.NVIC.icer[1].write(0xFFFF_FFFFu32);
        cortex_m::interrupt::enable();
    }

    // Release everything from reset
    let mut syscon = unsafe { Syscon::reset_all_noncritical_peripherals() };

    // Now we just INVERT pio0_5 before jumping
    let iocon = unsafe { Iocon::steal() }.enabled(&mut syscon).release();
    iocon.pio0_5.modify(|_, w| w
        .invert().set_bit()
        .digimode().digital()
    );

    // Jump to bootrom
    unsafe { cortex_m::asm::bootload(0x03000000 as *const u32) }
}
