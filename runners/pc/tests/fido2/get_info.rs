//! GetInfo tests — validate authenticator capabilities and CTAP2 compliance.

use super::*;
use paste::paste;

struct InfoCheck {
    /// Documents which `GetInfo` aspect this entry checks (for readers; not read by code).
    #[allow(dead_code)]
    name: &'static str,
    check: fn(&ctap2::get_info::Response),
}

const INFO_CHECKS: &[InfoCheck] = &[
    InfoCheck {
        name: "fido_2_0",
        check: |i| assert!(i.versions.contains(&ctap2::get_info::Version::Fido2_0)),
    },
    InfoCheck {
        name: "fido_2_1",
        check: |i| {
            assert!(
                i.versions.contains(&ctap2::get_info::Version::Fido2_1)
                    || i.versions.contains(&ctap2::get_info::Version::Fido2_1Pre),
                "expected FIDO_2_1 or FIDO_2_1_PRE version tag"
            )
        },
    },
    InfoCheck {
        name: "u2f_v2",
        check: |i| assert!(i.versions.contains(&ctap2::get_info::Version::U2fV2)),
    },
    InfoCheck {
        name: "aaguid_16b",
        check: |i| assert_eq!(i.aaguid.len(), 16),
    },
    InfoCheck {
        name: "pin_proto_1",
        check: |i| assert!(i.pin_protocols.as_ref().unwrap().contains(&1)),
    },
    InfoCheck {
        name: "opt_rk",
        check: |i| assert!(i.options.as_ref().unwrap().rk),
    },
    InfoCheck {
        name: "opt_up",
        check: |i| assert!(i.options.as_ref().unwrap().up),
    },
    InfoCheck {
        name: "opt_client_pin",
        check: |i| assert!(i.options.as_ref().unwrap().client_pin.is_some()),
    },
    InfoCheck {
        name: "max_msg_size",
        check: |i| assert!(i.max_msg_size.unwrap_or(0) > 0),
    },
    InfoCheck {
        name: "cred_mgmt",
        check: |i| assert_eq!(i.options.as_ref().unwrap().cred_mgmt, Some(true)),
    },
    InfoCheck {
        name: "ext_cred_protect",
        check: |i| {
            assert!(i
                .extensions
                .as_ref()
                .unwrap()
                .contains(&ctap2::get_info::Extension::CredProtect))
        },
    },
    InfoCheck {
        name: "ext_hmac_secret",
        check: |i| {
            assert!(i
                .extensions
                .as_ref()
                .unwrap()
                .contains(&ctap2::get_info::Extension::HmacSecret))
        },
    },
];

macro_rules! info_tests {
    ($($idx:literal => $name:ident,)*) => {
        paste! { $(
            #[test]
            #[serial]
            fn [<info_ $name>]() {
                run_in_thread(|| {
                    with_authenticator!([<info_ $name>], |authn| {
                        let resp = authn.call_ctap2(&Request::GetInfo).expect("GetInfo failed");
                        match resp {
                            Response::GetInfo(info) => (INFO_CHECKS[$idx].check)(&info),
                            other => panic!("Expected GetInfo, got {:?}", other),
                        }
                    });
                });
            }
        )* }
    };
}

info_tests! {
     0 => fido_2_0,
     1 => fido_2_1,
     2 => u2f_v2,
     3 => aaguid,
     4 => pin_protocol,
     5 => rk,
     6 => up,
     7 => client_pin,
     8 => max_msg_size,
     9 => cred_mgmt,
    10 => ext_cred_protect,
    11 => ext_hmac_secret,
}
