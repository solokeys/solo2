//! Board bring-up for the nRF52840-DK runner.
//!
//! `init_board` constructs every hardware/driver object from the device
//! peripherals and returns them in [`BoardComponents`]. The RTIC `#[init]`
//! in `main.rs` keeps only what needs the RTIC context — the SysTick
//! monotonic, the keepalive channel, and the task spawns. Mirrors the
//! lpc55 runner's `runner::init_board` split.

use core::fmt::Write;

use apdu_dispatch::dispatch::ApduDispatch;
use apdu_dispatch::interchanges as apdu_interchanges;
use ctaphid_dispatch::{Channel as CtapChannel, DefaultDispatch as CtaphidDispatchDefault};
use interchange::Channel;
use littlefs2::fs::{Allocation, Filesystem};
use nrf52840_hal::{
    clocks::{Clocks, ExternalOscillator, Internal, LfOscStarted},
    rng::Rng,
    usbd::{UsbPeripheral, Usbd},
};
use nrf52840_pac::POWER;
use static_cell::StaticCell;
use usb_device::{
    bus::UsbBusAllocator,
    device::{UsbDevice, UsbDeviceBuilder, UsbVidPid},
};
#[cfg(feature = "ccid")]
use usbd_ccid::Ccid;
use usbd_ctaphid::CtapHid;

use crate::dispatch::Dispatch;
use crate::flash::InternalFlashStorage;
use crate::nfct::NfctDevice;
use crate::types::{Apps, Board, RunnerStore, Trussed, VolatileStorage};
use crate::CTAPHID_MESSAGE_SIZE;

pub type UsbClock = Clocks<ExternalOscillator, Internal, LfOscStarted>;
pub type UsbBus = Usbd<UsbPeripheral<'static>>;
pub type CtapHidClass = CtapHid<'static, 'static, 'static, UsbBus, CTAPHID_MESSAGE_SIZE>;
#[cfg(feature = "ccid")]
pub type CcidClass = Ccid<'static, 'static, UsbBus, { apdu_dispatch::interchanges::SIZE }>;
// CCID off: the contact smartcard interface is not enumerated, so the shared
// slot is a unit placeholder (never locked — its task accesses are cfg'd out).
#[cfg(not(feature = "ccid"))]
pub type CcidClass = ();

/// Everything `init_board` builds from the device peripherals. The RTIC
/// `#[init]` splits these across the `Shared` / `Local` resource structs.
pub struct BoardComponents {
    pub trussed: Trussed,
    pub apps: Apps,
    pub ctaphid_dispatch: CtaphidDispatchDefault<'static, 'static>,
    pub apdu_dispatch: ApduDispatch<'static>,
    pub nfc_apdu_rq: apdu_interchanges::Requester<'static>,
    pub usbd: UsbDevice<'static, UsbBus>,
    pub ctaphid: CtapHidClass,
    pub ccid: CcidClass,
    pub power: POWER,
}

pub fn init_board(dp: nrf52840_pac::Peripherals) -> BoardComponents {
    static CLOCKS: StaticCell<UsbClock> = StaticCell::new();
    let clocks = CLOCKS.init(Clocks::new(dp.CLOCK).start_lfclk().enable_ext_hfosc());

    // Post-mortem: read POWER.RESETREAS to learn why the chip last reset,
    // then write-1-to-clear so the NEXT boot only sees the NEXT reset cause.
    // Bit map (nRF52840 RM): bit 0 RESETPIN, 1 DOG (WDT), 2 SREQ
    // (SCB::sys_reset / lockup post-reset), 3 LOCKUP, 16+ wake-from-System-OFF.
    let reset_reas = dp.POWER.resetreas.read().bits();
    defmt::warn!("RESETREAS=0x{=u32:08x}", reset_reas);
    dp.POWER.resetreas.write(|w| unsafe { w.bits(0xFFFFFFFF) });

    // Power events for VBUS detection.
    dp.POWER.intenset.write(|w| {
        w.usbdetected()
            .set_bit()
            .usbpwrrdy()
            .set_bit()
            .usbremoved()
            .set_bit()
    });

    // USBD interrupt sources (nrf-usbd 0.1 doesn't unmask these).
    dp.USBD.intenset.write(|w| {
        w.usbreset()
            .set_bit()
            .usbevent()
            .set_bit()
            .sof()
            .set_bit()
            .ep0datadone()
            .set_bit()
            .ep0setup()
            .set_bit()
            .endepin0()
            .set_bit()
            .endepout0()
            .set_bit()
    });

    // Device UUID derived from FICR. Computed before the USB device is built
    // so it can be exposed as the (hex-encoded) USB serial number; solo2-cli
    // hex-decodes the serial to derive the device UUID and drops any device
    // whose serial isn't valid hex.
    let ficr = dp.FICR;
    let did0 = ficr.deviceid[0].read().bits();
    let did1 = ficr.deviceid[1].read().bits();
    let mut uuid = [0u8; 16];
    uuid[0..4].copy_from_slice(&did0.to_be_bytes());
    uuid[4..8].copy_from_slice(&did1.to_be_bytes());

    static SERIAL_STRING: StaticCell<heapless::String<32>> = StaticCell::new();
    let serial_string = SERIAL_STRING.init(heapless::String::new());
    for b in &uuid {
        write!(serial_string, "{:02x}", b).ok();
    }
    let serial_number: &'static str = serial_string.as_str();

    static USB_BUS: StaticCell<UsbBusAllocator<UsbBus>> = StaticCell::new();
    let usb_bus = USB_BUS.init(Usbd::new(UsbPeripheral::new(dp.USBD, clocks)));

    static CTAP_CHANNEL: CtapChannel<CTAPHID_MESSAGE_SIZE> = Channel::new();
    let (ctaphid_rq, ctaphid_rp) = CTAP_CHANNEL.split().unwrap();

    // APDU interchanges. NFC requester is fed by the t4t bridge in nfct.rs;
    // USB requester is fed by the CCID class.
    static NFC_APDU_CHANNEL: apdu_dispatch::interchanges::Channel = Channel::new();
    static USB_APDU_CHANNEL: apdu_dispatch::interchanges::Channel = Channel::new();
    let (nfc_apdu_rq, nfc_apdu_rp) = NFC_APDU_CHANNEL.split().unwrap();
    let (usb_apdu_rq, usb_apdu_rp) = USB_APDU_CHANNEL.split().unwrap();
    let apdu_dispatch = ApduDispatch::new(usb_apdu_rp, nfc_apdu_rp);

    // Brings up libnfc_t4t.a (`nfc_t4t_setup` + `nfc_t4t_emulation_start`).
    // The returned handle is zero-sized — all NFC state lives inside the
    // library's own statics — so we drop it on return.
    NfctDevice::new(
        dp.NFCT,
        [uuid[0], uuid[1], uuid[2], uuid[3], uuid[4], uuid[5]],
    );

    let mut ctaphid: CtapHidClass = CtapHid::new(usb_bus, ctaphid_rq, 0)
        .implements_ctap1()
        .implements_ctap2()
        .implements_wink();
    ctaphid.set_version(usbd_ctaphid::Version {
        major: 0,
        minor: 1,
        build: 0,
    });

    // Register the CCID USB class: the host enumerates a smartcard reader
    // interface and contact APDUs (PIV/opcard/secrets) flow through
    // apdu_dispatch alongside the contactless NFC path.
    #[cfg(feature = "ccid")]
    let ccid: CcidClass = Ccid::new(usb_bus, usb_apdu_rq, Some(b"solo2-nrf"));
    // CCID off: don't enumerate the contact interface. The contact APDU
    // requester is dropped (apdu_dispatch keeps the responder half so the
    // contactless NFC path still works).
    #[cfg(not(feature = "ccid"))]
    let ccid: CcidClass = {
        let _ = usb_apdu_rq;
    };

    let ctaphid_dispatch = CtaphidDispatchDefault::new(ctaphid_rp);

    let usbd = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x1209, 0xbeee))
        .manufacturer("SoloKeys (port)")
        .product("solo2-nrf52840dk")
        .serial_number(serial_number)
        .device_release(0x0001)
        .max_packet_size_0(64)
        .composite_with_iads()
        .build();

    let (leds, buttons) = crate::board::init(dp.P0);

    // Storage:
    //   - `ifs` and `efs` are aliased to the same NVMC-backed filesystem
    //     (256 KiB at 0x000C_0000). PIV / opcard hardcode some backup paths
    //     to `Location::External`; pointing efs at the same flash FS makes
    //     those persist alongside the apps' main state.
    //   - `vfs` stays RAM-only (8 KiB) for true scratch.
    static INTERNAL_STORAGE: StaticCell<InternalFlashStorage> = StaticCell::new();
    static INTERNAL_FS_ALLOC: StaticCell<Allocation<InternalFlashStorage>> = StaticCell::new();
    static VOLATILE_STORAGE: StaticCell<VolatileStorage> = StaticCell::new();
    static VOLATILE_FS_ALLOC: StaticCell<Allocation<VolatileStorage>> = StaticCell::new();

    let internal_storage =
        INTERNAL_STORAGE.init(InternalFlashStorage::new(dp.NVMC)) as *mut InternalFlashStorage;
    let internal_alloc =
        INTERNAL_FS_ALLOC.init(Filesystem::allocate()) as *mut Allocation<InternalFlashStorage>;
    let volatile_storage = VOLATILE_STORAGE.init(VolatileStorage::new()) as *mut VolatileStorage;
    let volatile_alloc =
        VOLATILE_FS_ALLOC.init(Filesystem::allocate()) as *mut Allocation<VolatileStorage>;

    // Try to mount internal FS without formatting; format both on failure.
    let needs_format = Filesystem::mount(unsafe { &mut *internal_alloc }, unsafe {
        &mut *internal_storage
    })
    .is_err();

    if needs_format {
        // Wipes every persistent app state (oath secrets, FIDO credentials,
        // …). Surfaces in RTT so a silent reformat doesn't go unnoticed.
        defmt::warn!("littlefs2 IFS mount FAILED — reformatting IFS + VFS");
        Filesystem::format(unsafe { &mut *internal_storage }).unwrap();
        Filesystem::format(unsafe { &mut *volatile_storage }).unwrap();
    }

    static INTERNAL_FS: StaticCell<Filesystem<'static, InternalFlashStorage>> = StaticCell::new();
    let internal_fs = INTERNAL_FS.init(
        Filesystem::mount(unsafe { &mut *internal_alloc }, unsafe {
            &mut *internal_storage
        })
        .unwrap(),
    );

    static VOLATILE_FS: StaticCell<Filesystem<'static, VolatileStorage>> = StaticCell::new();
    let volatile_fs = VOLATILE_FS.init({
        match Filesystem::mount(unsafe { &mut *volatile_alloc }, unsafe {
            &mut *volatile_storage
        }) {
            Ok(fs) => fs,
            Err(_) => {
                Filesystem::format(unsafe { &mut *volatile_storage }).unwrap();
                Filesystem::mount(unsafe { &mut *volatile_alloc }, unsafe {
                    &mut *volatile_storage
                })
                .unwrap()
            }
        }
    });

    // efs aliased to the same NVMC filesystem as ifs.
    let store = RunnerStore::new(internal_fs, internal_fs, volatile_fs);

    // Run migrations on persistent state before any app touches the
    // filesystem. Idempotent: safe on every boot, no-op on already-migrated
    // state, no-op on a fresh device whose `fido/dat` directory does not yet
    // exist. Mirrors the lpc55 wiring.
    {
        use trussed::store::Store as _;
        let _ = fido_authenticator::state::migrate::migrate_no_rp_dir(
            store.ifs(),
            littlefs2::path!("fido/dat"),
        );
    }

    // Test-only FIDO2 attestation provisioning. Real firmware ships a
    // per-device attestation keypair installed at factory; this shortcut
    // bakes the public Nitrokey FIDO test PKI (same one `solo-pc` uses for
    // its sim runner) into the binary and writes it to LFS at boot whenever
    // it's missing. Without this, CTAP1 `Register` and CTAP2 `MakeCredential`
    // return `KeyReferenceNotFound (0x6A88)`. Gated by `test-up-control` so
    // production builds never include the test key.
    #[cfg(feature = "test-up-control")]
    {
        use trussed::store::Store as _;
        const ATTESTATION_CERT: &[u8] = include_bytes!("../../pc/data/fido-cert.der");
        const ATTESTATION_KEY: &[u8] = include_bytes!("../../pc/data/fido-key.trussed");
        let ifs = store.ifs();
        let _ = ifs.create_dir_all(littlefs2::path!("fido/x5c"));
        let _ = ifs.create_dir_all(littlefs2::path!("fido/sec"));
        let rc = ifs.write(littlefs2::path!("fido/x5c/00"), ATTESTATION_CERT);
        let rk = ifs.write(littlefs2::path!("fido/sec/00"), ATTESTATION_KEY);
        defmt::warn!(
            "test-up-control: prov FIDO attestation cert.write={=bool} key.write={=bool}",
            rc.is_ok(),
            rk.is_ok(),
        );
    }

    let dev_rng = Rng::new(dp.RNG);
    let board = Board::new(
        dev_rng,
        store,
        crate::board::UserInterface::new(leds, buttons),
    );
    let service = trussed::service::Service::with_dispatch(board, Dispatch::default());
    let mut trussed = Trussed::new(service);

    let version: u32 = 0;
    let apps = Apps::new(&mut trussed, uuid, version);

    BoardComponents {
        trussed,
        apps,
        ctaphid_dispatch,
        apdu_dispatch,
        nfc_apdu_rq,
        usbd,
        ctaphid,
        ccid,
        power: dp.POWER,
    }
}
