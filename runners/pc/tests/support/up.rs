//! Unified user presence control for tests.
//!
//! Selects backend based on environment:
//! - Default sim mode: in-process simulated buttons via `solo_pc::buttons` (`test-buttons`)
//! - `FIDO2_TRANSPORT=socket`: Unix socket side channel to the simulator process
//! - `UP_BACKEND=probe-rs`: shells out to `probe-rs write` for on-device testing
//!   Requires `UP_CONTROL_ADDR` (hex address) and `PROBE_RS_CHIP` to be set.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;

enum Backend {
    Socket,
    InProcess,
    ProbeRs {
        addr: String,
        chip: String,
        protocol: Option<String>,
        speed: Option<String>,
    },
}

fn backend() -> Backend {
    if std::env::var("UP_BACKEND").is_err()
        && std::env::var("FIDO2_TRANSPORT").as_deref() == Ok("socket")
    {
        return Backend::Socket;
    }

    match std::env::var("UP_BACKEND").as_deref() {
        Ok("probe-rs") => {
            let addr = std::env::var("UP_CONTROL_ADDR")
                .expect("UP_BACKEND=probe-rs requires UP_CONTROL_ADDR (e.g. 0x2003f000)");
            let chip = std::env::var("PROBE_RS_CHIP").unwrap_or_else(|_| "LPC55S69JBD100".into());
            let protocol = std::env::var("PROBE_RS_PROTOCOL").ok();
            let speed = std::env::var("PROBE_RS_SPEED").ok();
            Backend::ProbeRs {
                addr,
                chip,
                protocol,
                speed,
            }
        }
        _ => Backend::InProcess,
    }
}

fn socket_write(value: u8) {
    let mut attempts = 0;
    loop {
        match UnixStream::connect(solo_pc::SIM_UP_SOCKET_PATH) {
            Ok(mut stream) => {
                stream
                    .write_all(&[value])
                    .expect("failed to write UP socket command");
                let mut ack = [0u8; 1];
                stream
                    .read_exact(&mut ack)
                    .expect("failed to read UP socket ack");
                assert_eq!(ack[0], 0x00, "unexpected UP socket ack");
                return;
            }
            Err(err) if attempts < 20 => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
                if attempts == 20 {
                    panic!("failed to connect to UP socket: {}", err);
                }
            }
            Err(err) => panic!("failed to connect to UP socket: {}", err),
        }
    }
}

fn probe_rs_write(addr: &str, chip: &str, protocol: Option<&str>, speed: Option<&str>, value: u8) {
    let mut command = Command::new("probe-rs");
    command.args(["write", "b8", addr, &value.to_string(), "--chip", chip]);
    if let Some(protocol) = protocol {
        command.args(["--protocol", protocol]);
    }
    if let Some(speed) = speed {
        command.args(["--speed", speed]);
    }
    let status = command.status().expect("failed to run probe-rs");
    assert!(status.success(), "probe-rs write failed");
}

/// Approve the next user presence check.
pub fn approve() {
    match backend() {
        Backend::Socket => socket_write(1),
        Backend::InProcess => solo_pc::buttons::approve(),
        Backend::ProbeRs {
            addr,
            chip,
            protocol,
            speed,
        } => probe_rs_write(&addr, &chip, protocol.as_deref(), speed.as_deref(), 1),
    }
}

/// Approve all user presence checks until `reset()`.
pub fn approve_sticky() {
    match backend() {
        Backend::Socket => socket_write(129),
        Backend::InProcess => solo_pc::buttons::approve_sticky(),
        Backend::ProbeRs {
            addr,
            chip,
            protocol,
            speed,
        } => probe_rs_write(&addr, &chip, protocol.as_deref(), speed.as_deref(), 129),
    }
}

/// Deny all user presence checks (will timeout).
pub fn deny() {
    match backend() {
        Backend::Socket => socket_write(128),
        Backend::InProcess => solo_pc::buttons::deny(),
        Backend::ProbeRs {
            addr,
            chip,
            protocol,
            speed,
        } => probe_rs_write(&addr, &chip, protocol.as_deref(), speed.as_deref(), 128),
    }
}

/// Clear any pending UP response.
pub fn reset() {
    match backend() {
        Backend::Socket => socket_write(0),
        Backend::InProcess => solo_pc::buttons::reset(),
        Backend::ProbeRs {
            addr,
            chip,
            protocol,
            speed,
        } => probe_rs_write(&addr, &chip, protocol.as_deref(), speed.as_deref(), 0),
    }
}
