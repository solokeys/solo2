#[macro_use]
extern crate log;

mod cli;

use anyhow::anyhow;
use solo2::{Device, Select as _, Solo2, Uuid, UuidSelectable};

fn main() {
    restore_cursor_on_ctrl_c();

    use clap::Parser;
    let args = cli::Cli::parse();

    pretty_env_logger::formatted_builder()
        .filter(None, args.global_options.verbose.log_level_filter())
        .init();

    if let Err(err) = try_main(args) {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}

fn try_main(args: cli::Cli) -> anyhow::Result<()> {
    let uuid: Option<Uuid> = args
        .global_options
        .uuid
        .as_deref()
        .map(str::parse)
        .transpose()?;

    if args.global_options.ctap {
        Solo2::prefer_ctap();
    }
    if args.global_options.pcsc {
        Solo2::prefer_pcsc();
    }

    // use cli::Subcommands::*;
    use cli::Apps::*;
    match args.subcommand {
        cli::Subcommands::App(app) => {
            let solo2s: Vec<Solo2> =
                all_or_unwrap_or_interactively_select(uuid, args.global_options.all, "Solo 2")?;

            // let uuid = solo2.uuid();

            solo2s.into_iter().try_for_each(|mut solo2| {
                match &app {
                    Admin(admin) => {
                        use cli::Admin::*;
                        use solo2::apps::Admin;

                        let mut app = Admin::select(&mut solo2)?;

                        match admin {
                            Aid => {
                                println!("{}", hex::encode(Admin::application_id()).to_uppercase());
                                return Ok(());
                            }
                            Locked => {
                                let locked = app.locked()?;
                                println!("locked: {}", locked);
                            }
                            Restart => {
                                info!("attempting restart of devices");
                                app.reboot()?;
                            }
                            Maintenance => {
                                // TODO: figure out the correct solution
                                #[allow(clippy::drop_non_drop)]
                                drop(app);
                                println!("Tap button on key to reboot into bootloader/maintenance mode, or replug to abort...");
                                solo2.into_lpc55()?;
                            }
                            Uuid => {
                                let uuid = app.uuid()?;
                                println!("{:X}", uuid.simple());
                            }
                            Version => {
                                let version = app.version()?;
                                println!("{}", version.to_calver());
                            }
                            Wink => {
                                app.wink()?;
                            }
                            Config { cmd } => {
                                use cli::AdminConfig::*;
                                match cmd {
                                    Get { key } => println!("{}", app.get_config(key)?),
                                    Set { key, value } => {
                                        app.set_config(key, value)?;
                                        println!("set {key} = {value}");
                                    }
                                }
                            }
                            Set { cmd } => {
                                use cli::AdminSet::*;
                                match cmd {
                                    Led { idle, up, default } => {
                                        let (i, u) = if *default {
                                            (Admin::DEFAULT_LED_IDLE, Admin::DEFAULT_LED_UP)
                                        } else {
                                            let rgb = |s: &str| -> anyhow::Result<u32> {
                                                u32::from_str_radix(s.trim_start_matches("0x"), 16)
                                                    .map_err(|_| anyhow!("invalid RRGGBB hex: {}", s))
                                            };
                                            let need = || {
                                                anyhow!("need <IDLE> <UP> as RRGGBB hex, or --default")
                                            };
                                            let idle = idle.as_deref().ok_or_else(need)?;
                                            let up = up.as_deref().ok_or_else(need)?;
                                            (rgb(idle)?, rgb(up)?)
                                        };
                                        app.set_led(i, u)?;
                                        println!(
                                            "LED set: idle {:06x}, up {:06x} — device rebooted to apply",
                                            i & 0xff_ffff,
                                            u & 0xff_ffff
                                        );
                                    }
                                    Usb { vid, pid, manufacturer, product, default } => {
                                        let (v, p, m, pr) = if *default {
                                            (
                                                Admin::DEFAULT_USB_VID,
                                                Admin::DEFAULT_USB_PID,
                                                String::new(),
                                                String::new(),
                                            )
                                        } else {
                                            let u16hex = |s: &str| -> anyhow::Result<u16> {
                                                u16::from_str_radix(s.trim_start_matches("0x"), 16)
                                                    .map_err(|_| anyhow!("invalid hex: {}", s))
                                            };
                                            (
                                                vid.as_deref()
                                                    .map(u16hex)
                                                    .transpose()?
                                                    .unwrap_or(Admin::DEFAULT_USB_VID),
                                                pid.as_deref()
                                                    .map(u16hex)
                                                    .transpose()?
                                                    .unwrap_or(Admin::DEFAULT_USB_PID),
                                                manufacturer.clone().unwrap_or_default(),
                                                product.clone().unwrap_or_default(),
                                            )
                                        };
                                        app.set_usb(v, p, &m, &pr)?;
                                        println!("USB set: {v:04x}:{p:04x} — device rebooted to apply");
                                    }
                                }
                            }
                        }
                        Ok(())
                    }
                    Fido(fido) => {
                        use cli::Fido::*;
                        use solo2::apps::Fido;

                        let app = Fido::from(solo2.as_ctap_mut().ok_or_else(|| anyhow!("CTAP unavailable"))?);

                        match fido {
                            Init => {
                                println!("{:?}", app.init()?);
                            }
                            Wink => {
                                let channel = app.init()?.channel;
                                app.wink(channel)?;
                            }
                        }
                        Ok(())
                    }
                    Ndef(ndef) => {
                        use cli::Ndef::*;
                        use solo2::apps::Ndef;

                        let mut app = Ndef::select(&mut solo2)?;

                        match ndef {
                            Aid => {
                                println!("{}", hex::encode(Ndef::application_id()).to_uppercase());
                                return Ok(());
                            }
                            Capabilities => {
                                let capabilities = app.capabilities()?;
                                println!("{}", hex::encode(capabilities));
                            }
                            Data => {
                                let data = app.data()?;
                                println!("{}", hex::encode(data));
                            }
                        }
                        Ok(())
                    }
                    Oath(oath) => {
                        use cli::Oath::*;
                        use solo2::apps::Oath;

                        let mut app = Oath::select(&mut solo2)?;

                        match oath {
                            Aid => {
                                println!("{}", hex::encode(Oath::application_id()).to_uppercase());
                                Ok(())
                            }
                            Delete { label } => {
                                app.delete(label.clone())?;
                                Ok(())
                            }
                            List => {
                                let labels = app.list()?;
                                for label in labels {
                                    println!("{}", label);
                                }
                                Ok(())
                            }
                            Register(args) => {
                                use solo2::apps::oath;
                                use cli::{OathAlgorithm, OathKind};

                                // Either import from an otpauth:// URI, or build
                                // from the positional/flag arguments.
                                let mut credential = if let Some(uri) = &args.uri {
                                    oath::Credential::from_uri(uri)?
                                } else {
                                    let label = args.label.clone().ok_or_else(|| {
                                        anyhow::anyhow!("provide a label and secret, or use --uri")
                                    })?;
                                    let secret32 = args.secret.clone().ok_or_else(|| {
                                        anyhow::anyhow!("provide a secret, or use --uri")
                                    })?;
                                    let digest = match args.algorithm {
                                        OathAlgorithm::Sha1 => oath::Digest::Sha1,
                                        OathAlgorithm::Sha256 => oath::Digest::Sha256,
                                        OathAlgorithm::Sha512 => oath::Digest::Sha512,
                                    };
                                    let secret = oath::Secret::from_base32(&secret32, digest)?;
                                    let kind = match args.kind {
                                        OathKind::Hotp => oath::Kind::Hotp(oath::Hotp {
                                            initial_counter: args.counter,
                                        }),
                                        OathKind::Totp => oath::Kind::Totp(oath::Totp {
                                            period: args.period,
                                        }),
                                    };
                                    oath::Credential {
                                        label,
                                        issuer: args.issuer.clone(),
                                        secret,
                                        kind,
                                        algorithm: digest,
                                        digits: args.digits,
                                        touch_required: false,
                                        pin_protected: false,
                                    }
                                };
                                // Flags augment whatever was parsed (URI included).
                                if args.touch {
                                    credential.touch_required = true;
                                }
                                if args.protect_with_pin {
                                    credential.pin_protected = true;
                                }
                                if credential.digits < 6 || credential.digits > 8 {
                                    return Err(anyhow::anyhow!("Invalid number of OATH digits"));
                                }
                                let credential_id = app.register(credential)?;
                                println!("{}", credential_id);
                                Ok(())
                            }
                            Reset => app.reset(),
                            Rename { label, new_label } => {
                                app.rename(label, new_label)?;
                                println!("renamed {} -> {}", label, new_label);
                                Ok(())
                            }
                            Update { label, touch, no_touch } => {
                                if !touch && !no_touch {
                                    return Err(anyhow::anyhow!(
                                        "specify --touch or --no-touch"
                                    ));
                                }
                                app.set_touch(label, *touch)?;
                                println!(
                                    "{}: touch {}",
                                    label,
                                    if *touch { "required" } else { "not required" }
                                );
                                Ok(())
                            }
                            Verify { label, code } => {
                                app.verify_code(label, *code)?;
                                println!("code verified for {}", label);
                                Ok(())
                            }
                            Get { label } => {
                                let info = app.get(label)?;
                                if let Some(k) = &info.kind {
                                    println!("kind:      {}", k);
                                }
                                if let Some(a) = &info.algorithm {
                                    println!("algorithm: {}", a);
                                }
                                if let Some(l) = &info.login {
                                    println!("login:     {}", l);
                                }
                                if let Some(p) = &info.password {
                                    println!("password:  {}", p);
                                }
                                if let Some(m) = &info.metadata {
                                    println!("metadata:  {}", m);
                                }
                                Ok(())
                            }
                            SetPin { pin } => {
                                app.set_pin(pin)?;
                                println!("PIN set");
                                Ok(())
                            }
                            ChangePin { pin, new_pin } => {
                                app.change_pin(pin, new_pin)?;
                                println!("PIN changed");
                                Ok(())
                            }
                            VerifyPin { pin } => {
                                app.verify_pin(pin)?;
                                println!("PIN verified");
                                Ok(())
                            }
                            // TODO: factor out the conversion
                            Totp { label, timestamp, period } => {
                                use solo2::apps::oath;
                                use std::time::SystemTime;

                                let timestamp = timestamp
                                    .clone()
                                    .map(|s| s.parse())
                                    .transpose()?
                                    .unwrap_or_else(|| {
                                        let since_epoch = SystemTime::now()
                                            .duration_since(SystemTime::UNIX_EPOCH)
                                            .unwrap();
                                        since_epoch.as_secs()
                                    });
                                let authenticate = oath::Authenticate {
                                    label: label.clone(),
                                    timestamp,
                                    period: *period,
                                };
                                let code = app.authenticate(authenticate)?;
                                println!("{}", code);
                                Ok(())
                            }
                        }
                    }
                    OathMigrate(cmd) => {
                        use cli::OathMigrate::*;
                        use solo2::apps::OathMigrate;

                        let mut app = OathMigrate::select(&mut solo2)?;

                        match cmd {
                            Aid => {
                                println!(
                                    "{}",
                                    hex::encode(OathMigrate::application_id()).to_uppercase()
                                );
                                Ok(())
                            }
                            Count => {
                                println!("{}", app.count()?);
                                Ok(())
                            }
                            Export { output } => {
                                eprintln!("Touch the device to authorize the export...");
                                let lines = app.export()?;
                                match output {
                                    Some(path) => {
                                        std::fs::write(path, &lines)?;
                                        eprintln!(
                                            "wrote {} bytes of otpauth:// lines to {}",
                                            lines.len(),
                                            path.display()
                                        );
                                    }
                                    None => {
                                        use std::io::Write as _;
                                        std::io::stdout().write_all(&lines)?;
                                    }
                                }
                                Ok(())
                            }
                            Delete => {
                                println!(
                                    "Touch the device to authorize deleting the legacy store..."
                                );
                                app.delete()?;
                                println!("deleted legacy /oath store");
                                Ok(())
                            }
                        }
                    }
                    Piv(piv) => {
                        use cli::Piv::*;
                        use solo2::apps::Piv;

                        // let mut app = Piv::select(&mut solo2)?;
                        Piv::select(&mut solo2)?;

                        match piv {
                            Aid => {
                                println!("{}", hex::encode(Piv::application_id()).to_uppercase());
                                Ok(())
                            }
                        }
                    }
                    Provision(provision) => {
                        use cli::Provision::*;
                        use solo2::apps::provision::App as Provision;

                        let mut app = Provision::select(&mut solo2)?;

                        match provision {
                            Aid => {
                                println!(
                                    "{}",
                                    hex::encode(Provision::application_id()).to_uppercase()
                                );
                                return Ok(());
                            }
                            GenerateEd255Key => {
                                let public_key = app.generate_trussed_ed255_attestation_key()?;
                                println!("{}", hex::encode(public_key));
                            }
                            GenerateP256Key => {
                                let public_key = app.generate_trussed_p256_attestation_key()?;
                                println!("{}", hex::encode(public_key));
                            }
                            GenerateX255Key => {
                                let public_key = app.generate_trussed_x255_attestation_key()?;
                                println!("{}", hex::encode(public_key));
                            }
                            ReformatFilesystem => app.reformat_filesystem()?,
                            StoreEd255Cert { der } => {
                                let certificate = std::fs::read(der)?;
                                app.store_trussed_ed255_attestation_certificate(&certificate)?;
                            }
                            StoreP256Cert { der } => {
                                let certificate = std::fs::read(der)?;
                                app.store_trussed_p256_attestation_certificate(&certificate)?;
                            }
                            StoreX255Cert { der } => {
                                let certificate = std::fs::read(der)?;
                                app.store_trussed_x255_attestation_certificate(&certificate)?;
                            }
                            StoreT1Pubkey { bytes } => {
                                let pubkey: [u8; 32] = std::fs::read(bytes)?.as_slice().try_into()?;
                                app.store_trussed_t1_intermediate_public_key(pubkey)?;
                            }
                            StoreFidoBatchCert { cert } => {
                                let cert = std::fs::read(cert)?;
                                app.write_file(&cert, "/fido/x5c/00")?;
                            }
                            StoreFidoBatchKey { bytes } => {
                                let key = std::fs::read(bytes)?;
                                app.write_file(&key, "/fido/sec/00")?;
                            }
                            WriteFile { data, path } => {
                                let data = std::fs::read(data)?;
                                app.write_file(&data, path)?;
                            }
                        }
                        Ok(())
                    }
                    Qa(cmd) => {
                        use cli::Qa::*;
                        use solo2::apps::qa::App;

                        App::select(&mut solo2)?;

                        match cmd {
                            Aid => {
                                println!("{}", hex::encode(App::application_id()).to_uppercase());
                            }
                        }
                        Ok(())
                    }
                    Wallet(cmd) => {
                        use cli::Wallet::*;
                        use solo2::apps::wallet::Chain;

                        let mut wallet = solo2::apps::Wallet::open(solo2.uuid())?;
                        match cmd {
                            Pubkey { eth, path, .. } => {
                                let chain = if *eth { Chain::Eth } else { Chain::Sol };
                                println!("{}", wallet.pubkey(chain, path.as_deref())?);
                            }
                            Reset => {
                                wallet.reset()?;
                                println!("Secret reset to zero private key");
                            }
                            Keygen { silent } => {
                                let words = wallet.keygen(!silent)?;
                                if !silent {
                                    println!("Generated seed. Write down these 24 words:");
                                    for (i, word) in words.iter().enumerate() {
                                        print!("{} ", word);
                                        if (i + 1) % 6 == 0 {
                                            println!();
                                        }
                                    }
                                    if words.len() % 6 != 0 {
                                        println!();
                                    }
                                }
                            }
                            Seed { read, words } => {
                                if *read {
                                    println!("{}", wallet.seed_read()?);
                                } else if words.len() != 24 {
                                    return Err(anyhow!(
                                        "Seed must be 24 words, got {}",
                                        words.len()
                                    ));
                                } else {
                                    wallet.seed(words.clone())?;
                                    println!("Seed set successfully");
                                }
                            }
                            Privkey { file } => {
                                wallet.privkey(file)?;
                                println!("Private key set successfully");
                            }
                            SetChain { eth, .. } => {
                                let chain = if *eth { Chain::Eth } else { Chain::Sol };
                                wallet.set_chain(chain)?;
                                println!(
                                    "Active chain: {}",
                                    if *eth { "Ethereum" } else { "Solana" }
                                );
                            }
                        }
                        Ok(())
                    }
                }
            })?;
        }
        cli::Subcommands::Pki(pki) => {
            match pki {
                cli::Pki::Ca(ca) => match ca {
                    cli::Ca::FetchCertificate { authority } => {
                        use std::io::{stdout, IsTerminal as _, Write as _};
                        let authority: solo2::pki::Authority = authority.as_str().try_into()?;
                        let certificate = solo2::pki::fetch_certificate(authority)?;
                        if stdout().is_terminal() {
                            eprintln!("Some things to do with the DER data");
                            eprintln!(
                                "* redirect to a file: `> {}.der`",
                                &authority.name().to_lowercase()
                            );
                            eprintln!("* inspect contents by piping to step: `| step certificate inspect`");
                            return Err(anyhow::anyhow!("Refusing to write binary data to stdout"));
                        }
                        stdout().write_all(certificate.der())?;
                    }
                },
                #[cfg(feature = "dev-pki")]
                cli::Pki::Dev(dev) => match dev {
                    cli::Dev::Fido { key, cert } => {
                        let (aaguid, key_trussed, key_pem, certificate) =
                            solo2::pki::dev::generate_selfsigned_fido();

                        info!("\n{}", key_pem);
                        info!("\n{}", certificate.serialize_pem()?);

                        std::fs::write(key, key_trussed)?;
                        std::fs::write(cert, &certificate.serialize_der()?)?;

                        println!("{}", hex::encode_upper(aaguid));
                    }
                },
                cli::Pki::Web => {
                    let solo2: Solo2 = unwrap_or_interactively_select(uuid, "Solo 2")?;
                    let uuid = solo2.uuid().simple();
                    let url = format!("https://s2pki.net/s2/{uuid}/x255.txt");
                    println!("=> {}", url);
                    webbrowser::open(&url)?;
                }
            }
        }
        cli::Subcommands::Bootloader(args) => match args {
            cli::Bootloader::Reboot => {
                let bootloader = match uuid {
                    Some(uuid) => lpc55::Bootloader::having(uuid)?,
                    None => interactively_select(lpc55::Bootloader::list(), "Solo 2 bootloaders")?,
                };
                bootloader.reboot();
            }
            cli::Bootloader::List => {
                let bootloaders = lpc55::Bootloader::list();
                for bootloader in bootloaders {
                    println!("{}", &Device::Lpc55(bootloader));
                }
            }
        },
        cli::Subcommands::Completion(args) => {
            use clap::CommandFactory as _;
            use clap_complete::{generate, shells::*};
            use std::io::stdout;
            let mut app = cli::Cli::command();
            match args {
                cli::Completion::Bash => generate(Bash, &mut app, "solo2", &mut stdout()),
                cli::Completion::Fish => generate(Fish, &mut app, "solo2", &mut stdout()),
                cli::Completion::PowerShell => {
                    generate(PowerShell, &mut app, "solo2", &mut stdout())
                }
                cli::Completion::Zsh => generate(Zsh, &mut app, "solo2", &mut stdout()),
            }
        }
        cli::Subcommands::List => {
            let devices = solo2::Device::list();
            for device in devices {
                println!("{}", &device);
            }
        }
        cli::Subcommands::Update {
            dry_run,
            yes,
            all,
            with,
        } => {
            let firmware: solo2::Firmware = with
                .map(solo2::Firmware::read_from_file)
                .unwrap_or_else(|| {
                    println!("Downloading latest release from https://github.com/solokeys/solo2/");
                    solo2::Firmware::download_latest()
                })?;

            println!(
                "Fetched firmware version {} ({})",
                &firmware.version().to_calver(),
                &firmware.version().to_semver(),
            );

            if dry_run {
                return Ok(());
            }

            if all {
                for device in Device::list() {
                    let bar = indicatif::ProgressBar::new(firmware.len() as u64);
                    let progress = |bytes: usize| bar.set_position(bytes as u64);
                    device.program(firmware.clone(), yes, Some(&progress))?;
                }
                return Ok(());
            } else {
                let device = match uuid {
                    Some(uuid) => Device::having(uuid)?,
                    None => interactively_select(Device::list(), "Solo 2 devices")?,
                };
                let bar = indicatif::ProgressBar::new(firmware.len() as u64);
                let progress = |bytes: usize| bar.set_position(bytes as u64);
                return device.program(firmware, yes, Some(&progress));
            }
        }
    }

    Ok(())
}

/// description: plural of thing to be selected, e.g. "Solo 2 devices"
pub fn interactively_select<T: core::fmt::Display>(
    candidates: Vec<T>,
    description: &str,
) -> anyhow::Result<T> {
    let mut candidates = match candidates.len() {
        0 => return Err(anyhow!("Empty list of {}", description)),
        1 => {
            let mut candidates = candidates;
            return Ok(candidates.remove(0));
        }
        _ => candidates,
    };

    let items: Vec<String> = candidates
        .iter()
        .map(|candidate| format!("{}", &candidate))
        .collect();

    use dialoguer::{theme, Select};
    // let selection = Select::with_theme(&theme::SimpleTheme)
    let selection = Select::with_theme(&theme::ColorfulTheme::default())
        .with_prompt(format!(
            "Multiple {} available, select one or hit Escape key",
            description
        ))
        .items(&items)
        .default(0)
        .interact_opt()?
        .ok_or_else(|| anyhow!("No candidate selected"))?;

    Ok(candidates.remove(selection))
}

pub fn all_or_unwrap_or_interactively_select<T: core::fmt::Display + UuidSelectable>(
    uuid: Option<Uuid>,
    all: bool,
    description: &str,
) -> anyhow::Result<Vec<T>> {
    let thing = match uuid {
        Some(uuid) => vec![T::having(uuid)?],
        None => match all {
            true => T::list(),
            false => vec![interactively_select(T::list(), description)?],
        },
    };
    Ok(thing)
}

pub fn unwrap_or_interactively_select<T: core::fmt::Display + UuidSelectable>(
    uuid: Option<Uuid>,
    description: &str,
) -> anyhow::Result<T> {
    let thing = match uuid {
        Some(uuid) => T::having(uuid)?,
        None => interactively_select(T::list(), description)?,
    };
    Ok(thing)
}

/// In `dialoguer` dialogs, the cursor is hidden and, if the user interrupts via Ctrl-C,
/// not shown again (for reasons). This is a best effort attempt to show the cursor again
/// in these situations.
fn restore_cursor_on_ctrl_c() {
    ctrlc::set_handler(move || {
        let term = dialoguer::console::Term::stderr();
        term.show_cursor().ok();
        // Ctrl-C exit code = 130
        std::process::exit(130);
    })
    .ok();
}
