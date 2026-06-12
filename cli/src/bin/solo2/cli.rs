use clap::{self, crate_authors, crate_version, Args, Parser, Subcommand, ValueEnum};

/// CLI to update and use Solo 2 security keys.
///
/// Print more logs by adding `-v` or `-vv`.
///
/// Project homepage: <https://github.com/solokeys/solo2-cli>
///
/// Trussed homepage: <https://trussed.dev>

//
// Design: [Rain's Rust CLI recommendations][cli-recommendations] is a good read.
//
// [cli-recommendations]: https://rust-cli-recommendations.sunshowers.io/

#[derive(Parser)]
#[clap(infer_subcommands = true)]
#[clap(author = crate_authors!())]
#[clap(version = crate_version!())]
pub struct Cli {
    #[clap(flatten)]
    pub global_options: GlobalOptions,
    #[clap(subcommand)]
    pub subcommand: Subcommands,
}

#[derive(Debug, Args)]
pub struct GlobalOptions {
    /// Prefer CTAP transport.
    #[clap(global = true, help_heading = "TRANSPORT", long)]
    pub ctap: bool,

    /// Prefer PCSC transport.
    #[clap(global = true, help_heading = "TRANSPORT", long)]
    pub pcsc: bool,

    /// Specify UUID of a Solo 2 device.
    #[clap(global = true, help_heading = "SELECTION", long, short)]
    pub uuid: Option<String>,

    /// Interact with all applicable Solo 2 devices.
    #[clap(
        global = true,
        help_heading = "SELECTION",
        long,
        // would conflict with OATH's algorithm flag
        // short,
        conflicts_with = "uuid"
    )]
    pub all: bool,

    /// Verbosity level (can be specified multiple times)
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity<clap_verbosity_flag::WarnLevel>,
}

#[derive(Subcommand)]
pub enum Subcommands {
    #[clap(subcommand)]
    App(Apps),

    #[clap(subcommand)]
    Bootloader(Bootloader),

    #[clap(subcommand)]
    Completion(Completion),

    /// List all available devices
    #[clap(visible_alias = "ls")]
    List,

    #[clap(subcommand)]
    Pki(Pki),

    /// Update to latest firmware published by SoloKeys. Warns on Major updates.
    Update {
        /// Just show the version that would be installed
        #[clap(long, short = 'n')]
        dry_run: bool,
        /// DANGER! Proceed with major updates without prompt
        #[clap(long, short)]
        yes: bool,
        /// Update all connected SoloKeys Solo 2 devices
        #[clap(long, short)]
        all: bool,
        /// Update to a specific firmware secure boot file (.sb2)
        #[clap(long, short)]
        with: Option<String>,
    },
}

#[derive(Subcommand)]
/// Interact with bootloader
pub enum Bootloader {
    /// List all available bootloaders
    #[clap(visible_alias = "ls")]
    List,
    // NB: If we convert lpc55-host to clap 3, should be possible
    // to slot in its CLI here.

    // /// Run a sequence of bootloader provision commands defined in the config file
    // Provision {
    //     /// Configuration file containing settings
    //     config: String,
    // },
    /// Reboots (into device if firmware is valid)
    Reboot,
}

#[derive(Subcommand)]
/// Generate shell completion scripts
pub enum Completion {
    /// Print completion script for Bash
    Bash,
    /// Print completion script for Fish
    Fish,
    /// Print completion script for PowerShell
    PowerShell,
    /// Print completion script for Zsh
    Zsh,
}

#[derive(Subcommand)]
/// PKI-related
pub enum Pki {
    #[clap(subcommand)]
    Ca(Ca),
    #[cfg(feature = "dev-pki")]
    #[clap(subcommand)]
    Dev(Dev),
    Web,
}

#[derive(Subcommand)]
/// CA-related
pub enum Ca {
    /// Fetch one of the well-known Solo 2 PKI certificates in DER format
    FetchCertificate {
        /// Name of authority, e.g. R1, T1, S3, etc.
        authority: String,
    },
}

#[derive(Subcommand)]
/// PKI for development
pub enum Dev {
    /// Fetch one of the well-known Solo 2 PKI certificates in DER format
    Fido {
        /// Output file for private P256 key in binary format
        key: String,
        /// Output file for self-signed certificate in DER format
        cert: String,
    },
}

#[derive(Subcommand)]
#[clap(infer_subcommands = true)]
/// Interact with on-device applications
pub enum Apps {
    #[clap(subcommand)]
    Admin(Admin),
    #[clap(subcommand)]
    Fido(Fido),
    #[clap(subcommand)]
    Ndef(Ndef),
    #[clap(subcommand)]
    Oath(Oath),
    #[clap(subcommand)]
    OathMigrate(OathMigrate),
    #[clap(subcommand)]
    Piv(Piv),
    #[clap(subcommand)]
    Provision(Provision),
    #[clap(subcommand)]
    Qa(Qa),
}

#[derive(Subcommand)]
#[clap(infer_subcommands = true)]
/// admin app
pub enum Admin {
    /// Print the application's AID
    Aid,
    /// Is device locked? (not available in early firmware)
    Locked,
    /// Switch device to maintenance mode (reboot into LPC 55 bootloader)
    #[clap(alias = "boot-to-bootrom")]
    Maintenance,
    /// Reboot device (as Solo 2)
    #[clap(alias = "reboot")]
    Restart,
    /// Return device UUID (not available over CTAP in early firmware)
    Uuid,
    /// Return device firmware version
    Version,
    /// Wink the device
    Wink,
}

#[derive(Subcommand)]
#[clap(infer_subcommands = true)]
/// FIDO app
pub enum Fido {
    /// FIDO init response
    Init,
    /// FIDO wink
    Wink,
}

#[derive(Subcommand)]
#[clap(infer_subcommands = true)]
/// NDEF app
pub enum Ndef {
    /// Print the application's AID
    Aid,
    /// NDEF capabilities
    Capabilities,
    /// NDEF data
    Data,
}

#[derive(Subcommand)]
#[clap(infer_subcommands = true)]
/// OATH app
pub enum Oath {
    /// Print the application's AID
    Aid,
    // Authenticate,
    /// Delete existing credential
    Delete {
        /// Label of credential
        label: String,
    },
    /// List all credentials
    List,
    /// Register new credential (positional args or --uri)
    Register(OathRegister),
    /// Rename an existing credential
    Rename {
        /// current label
        label: String,
        /// new label
        new_label: String,
    },
    /// Change a credential's touch requirement
    Update {
        /// label of credential
        label: String,
        /// require a touch before generating a code
        #[clap(long)]
        touch: bool,
        /// remove the touch requirement
        #[clap(long, conflicts_with = "touch")]
        no_touch: bool,
    },
    /// Verify an incoming HOTP code (advances the counter on a match)
    Verify {
        /// label of credential
        label: String,
        /// the HOTP code to verify
        code: u32,
    },
    /// Show a credential's metadata and any Password Safe fields
    Get {
        /// label of credential
        label: String,
    },
    /// Set the device PIN (used by PIN-protected credentials)
    SetPin {
        /// the new PIN
        pin: String,
    },
    /// Change the device PIN
    ChangePin {
        /// current PIN
        pin: String,
        /// new PIN
        new_pin: String,
    },
    /// Verify the device PIN for this session
    VerifyPin {
        /// the PIN
        pin: String,
    },
    /// Reset OATH app, deleting all credentials
    Reset,
    /// Calculate TOTP for a registered credential
    Totp {
        /// Label of credential
        label: String,
        /// timestamp to use to generate the OTP, as seconds since the UNIX epoch
        timestamp: Option<String>,
        /// TOTP period in seconds (default 30)
        #[clap(long, default_value = "30")]
        period: u32,
    },
}

#[derive(Args)]
pub struct OathRegister {
    /// label to use for the OATH secret, e.g. alice@trussed.dev (omit with --uri)
    pub label: Option<String>,
    /// the actual OATH seed, e.g. JBSWY3DPEHPK3PXP (omit with --uri)
    pub secret: Option<String>,

    /// Import from an otpauth:// URI instead of the positional/flag arguments.
    ///
    /// Recognized: secret, algorithm, digits, period, counter, issuer, touch.
    /// Accepts unpadded base32 secrets.
    #[clap(long)]
    pub uri: Option<String>,

    /// (optional) issuer to use for the OATH credential, e.g., example.com
    #[clap(long, short)]
    pub issuer: Option<String>,

    #[clap(default_value = "sha1", long, short, value_enum)]
    pub algorithm: OathAlgorithm,
    #[clap(default_value = "totp", long, short, value_enum)]
    pub kind: OathKind,

    /// (only HOTP) initial counter to use for HOTPs
    #[clap(default_value = "0", long, short)] //, required_if_eq("kind", "hotp"))]
    pub counter: u32,

    /// number of digits to output
    #[clap(default_value = "6", long, short)]
    pub digits: u8,

    /// (only TOTP) period in seconds for which a TOTP is valid
    #[clap(default_value = "30", long, short)] //, required_if_eq("kind", "totp"))]
    pub period: u32,

    /// require a touch on the device before this credential generates a code
    #[clap(long)]
    pub touch: bool,

    /// encrypt the credential under the device PIN (PIN required before use)
    #[clap(long)]
    pub protect_with_pin: bool,
}

// ignore case?
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
/// hash algorithm to use in OTP generation
pub enum OathAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

// ignore case?
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
/// kind of OATH credential to register
pub enum OathKind {
    Hotp,
    Totp,
}

#[derive(Subcommand)]
#[clap(infer_subcommands = true)]
/// Migrate legacy (oath-authenticator 0.1) OATH credentials off the device.
///
/// Flow: `count` -> `export` -> verify/re-import the otpauth:// URIs -> `delete`.
pub enum OathMigrate {
    /// Print the application's AID
    Aid,
    /// Count legacy credentials still present on the device
    Count,
    /// Export legacy credentials as plaintext otpauth:// lines (requires a touch).
    ///
    /// Lines go to stdout by default — pipe them straight into re-import, or into
    /// `age -r <recipient>` to encrypt at rest. Use --output to write a file.
    Export {
        /// Write the otpauth:// lines to this file instead of stdout
        #[clap(long, short)]
        output: Option<std::path::PathBuf>,
    },
    /// Delete the legacy OATH store to reclaim flash (requires a touch)
    Delete,
}

#[derive(Subcommand)]
#[clap(infer_subcommands = true)]
/// PIV app
pub enum Piv {
    /// Print the application's AID
    Aid,
}

#[derive(Subcommand)]
#[clap(infer_subcommands = true)]
/// Provision app
pub enum Provision {
    /// Print the application's AID
    Aid,
    /// Generate new Trussed Ed255 attestation key
    GenerateEd255Key,
    /// Generate new Trussed P256 attestation key
    GenerateP256Key,
    /// Generate new Trussed X255 attestation key
    GenerateX255Key,

    /// Store Trussed Ed255 attestation certificate
    StoreEd255Cert {
        /// Certificate in DER format
        der: String,
    },
    /// Store Trussed P256 attestation certificate
    StoreP256Cert {
        /// Certificate in DER format
        der: String,
    },
    /// Store Trussed X255 attestation certificate
    StoreX255Cert {
        /// Certificate in DER format
        der: String,
    },

    /// Store Trussed T1 intermediate public key
    StoreT1Pubkey {
        /// Ed255 public key (raw, 32 bytes)
        bytes: String,
    },
    /// Store FIDO batch attestation certificate
    StoreFidoBatchCert {
        /// Attestation certificate
        cert: String,
    },
    /// Store FIDO batch attestation private key
    StoreFidoBatchKey {
        /// P256 private key in internal format
        bytes: String,
    },

    /// Reformat the internal filesystem
    ReformatFilesystem,

    /// Write binary file to specified path
    WriteFile {
        /// binary data file
        data: String,
        /// path in internal filesystem
        path: String,
    },
}

#[derive(Subcommand)]
#[clap(infer_subcommands = true)]
/// QA app
pub enum Qa {
    /// Print the application's AID
    Aid,
}

///// Return the "long" format of lpc55's version string.
/////
///// If a revision hash is given, then it is used. If one isn't given, then
///// the SOLO2_CLI_BUILD_GIT_HASH env var is inspected for it. If that isn't set,
///// then a revision hash is not included in the version string returned.
//pub fn long_version(revision_hash: Option<&str>) -> String {
//    // Do we have a git hash?
//    // (Yes, if ripgrep was built on a machine with `git` installed.)
//    let hash = match revision_hash.or(option_env!("SOLO2_CLI_BUILD_GIT_HASH")) {
//        None => String::new(),
//        Some(githash) => format!(" (rev {})", githash),
//    };
//    format!("{}{}", crate_version!(), hash)
//}
