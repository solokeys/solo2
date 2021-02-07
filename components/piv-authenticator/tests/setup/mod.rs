trussed::platform!(Platform,
    R: rand_core::OsRng,//chacha20::ChaCha8Rng,
    S: store::Store,
    UI: ui::UserInterface,
);

#[macro_export]
macro_rules! cmd {
    ($tt:tt) => { iso7816::Command::try_from(&hex_literal::hex!($tt)).unwrap() }
}

pub type Piv<'service> = piv_authenticator::Authenticator<trussed::ClientImplementation<&'service mut trussed::service::Service<Platform>>>;

pub fn piv<R>(test: impl FnOnce(&mut Piv) -> R) -> R {
    use trussed::Interchange as _;
    unsafe { trussed::pipe::TrussedInterchange::reset_claims(); }
    let trussed_platform = init_platform();
    let mut trussed_service = trussed::service::Service::new(trussed_platform);
    let client_id = "test";
    let trussed_client = trussed_service.try_as_new_client(client_id).unwrap();
    let mut piv_app = piv_authenticator::Authenticator::new(trussed_client);
    test(&mut piv_app)
}

pub fn init_platform() -> Platform {
    let rng = rand_core::OsRng;
    let store = store::Store::format(
        store::InternalStorage::new(),
        store::ExternalStorage::new(),
        store::VolatileStorage::new(),
        );
    let ui = ui::UserInterface::new();

    let platform = Platform::new(rng, store, ui);

    platform
}

pub mod ui {
    use trussed::platform::{consent, reboot, ui};
    pub struct UserInterface { start_time: std::time::Instant }

    impl UserInterface { pub fn new() -> Self { Self { start_time: std::time::Instant::now() } } }

    impl trussed::platform::UserInterface for UserInterface {
        fn check_user_presence(&mut self) -> consent::Level { consent::Level::Normal }
        fn set_status(&mut self, _status: ui::Status) {}
        fn refresh(&mut self) {}
        fn uptime(&mut self) -> core::time::Duration { self.start_time.elapsed() }
        fn reboot(&mut self, _to: reboot::To) -> ! { loop { continue; } }
    }
}

pub mod store {
    pub use heapless::consts;
    use littlefs2::{const_ram_storage, fs::{Allocation, Filesystem}};
    use trussed::types::{LfsResult, LfsStorage};

    const_ram_storage!(InternalStorage, 8192);
    const_ram_storage!(ExternalStorage, 8192);
    const_ram_storage!(VolatileStorage, 8192);

    trussed::store!(Store,
        Internal: InternalStorage,
        External: ExternalStorage,
        Volatile: VolatileStorage
    );
}
