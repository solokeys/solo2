use clap::Parser;
use clap_num::maybe_hex;

use dispatch_fido::Fido;
use interchange::Interchange;
use usb_device::{bus::UsbBusAllocator, prelude::*};
use usbip_device::UsbIpBus;

use solo_usbip::platform::init_platform;

/// USP/IP based virtualization of the Nitrokey 3 / Solo2 device.
/// Supports FIDO application at the moment.
#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    /// USB Name string
    #[clap(short, long, default_value = "FIDO authenticator")]
    name: String,

    /// USB Manufacturer string
    #[clap(short, long, default_value = "Simulation")]
    manufacturer: String,

    /// USB Serial string
    #[clap(long, default_value = "SIM SIM SIM")]
    serial: String,

    /// Trussed state file
    #[clap(long, default_value = "trussed-state.bin")]
    state_file: String,

    /// USB VID id
    #[clap(short, long, parse(try_from_str=maybe_hex), default_value_t = 0x20a0)]
    vid: u16,
    /// USB PID id
    #[clap(short, long, parse(try_from_str=maybe_hex), default_value_t = 0x42b2)]
    pid: u16,
}


fn main() {
    #[cfg(feature = "enable-logs")]
    pretty_env_logger::init();
    let args = Args::parse();

    log::info!("Initializing Trussed");
    let state_file: String = args.state_file;
    let trussed_platform = init_platform(state_file);
    let mut trussed_service = trussed::service::Service::new(trussed_platform);
    let client_id = "fido";
    let trussed_client = trussed_service.try_as_new_client(client_id).unwrap();

    log::info!("Initializing allocator");
    // To change IP or port see usbip-device-0.1.4/src/handler.rs:26
    let bus_allocator = UsbBusAllocator::new(UsbIpBus::new());
    let (ctaphid_rq, ctaphid_rp) = ctaphid_dispatch::types::HidInterchange::claim().unwrap();
    let mut ctaphid = usbd_ctaphid::CtapHid::new(&bus_allocator, ctaphid_rq, 0u32)
        .implements_ctap1()
        .implements_ctap2()
        .implements_wink();
    let mut ctaphid_dispatch = ctaphid_dispatch::dispatch::Dispatch::new(ctaphid_rp);
    let mut usb_bus = UsbDeviceBuilder::new(&bus_allocator, UsbVidPid(args.vid, args.pid))
        .manufacturer(&args.manufacturer)
        .product(&args.name)
        .serial_number(&args.serial)
        .device_class(0x03)
        .device_sub_class(0)
        .build();

    let fido_auth = fido_authenticator::Authenticator::new(trussed_client, fido_authenticator::NonSilentAuthenticator {});
    let mut fido_app = Fido::<fido_authenticator::NonSilentAuthenticator, _>::new(fido_auth);

    log::info!("Ready for work");
    loop {
        std::thread::sleep(std::time::Duration::from_millis(5));
        ctaphid_dispatch.poll(&mut [&mut fido_app]);
        usb_bus.poll(&mut [&mut ctaphid]);
    }
}
