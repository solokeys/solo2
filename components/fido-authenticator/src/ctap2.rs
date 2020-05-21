use cortex_m_semihosting::hprintln;

use littlefs2::{
    driver::Storage,
};

use usbd_ctaphid::{
    authenticator::{
        self,
        Error,
        Result,
    },
    constants::{
        MESSAGE_SIZE,
    },
    types::{
        ByteBuf,
        CtapOptions,
        consts,
        String,
        Vec,
        AssertionResponses,
        AttestationObject,
        AuthenticatorInfo,
        GetAssertionParameters,
        MakeCredentialParameters,
    },
};

use crate::constants::AAGUID;

impl<'fs, 'storage, R, S> authenticator::Api for crate::Authenticator<'fs, 'storage, R, S>
where
    R: embedded_hal::blocking::rng::Read,
    S: Storage,
{
    fn get_info(&mut self) -> AuthenticatorInfo {
        use core::str::FromStr;
        let mut versions = Vec::<String<consts::U12>, consts::U3>::new();
        versions.push(String::from_str("U2F_V2").unwrap()).unwrap();
        versions.push(String::from_str("FIDO_2_0").unwrap()).unwrap();
        versions.push(String::from_str("FIDO_2_1_PRE").unwrap()).unwrap();

        let mut extensions = Vec::<String<consts::U11>, consts::U4>::new();
        extensions.push(String::from_str("credProtect").unwrap()).unwrap();
        extensions.push(String::from_str("hmac-secret").unwrap()).unwrap();

        let options = CtapOptions {
            plat: false,
            rk: true,
            client_pin: Some(self.client_pin().is_some()),
            up: true,
            uv: None, // cannot perform UV within ourselves (e.g. biometrics)
            cred_protect: Some(true),
        };

        hprintln!("options = {:?}", &options).ok();

        self.authnr_channels.send.enqueue(
            crate::AuthnrToOsMessages::Heya(String::from_str("GET_INFO").unwrap()));
        crate::pac::NVIC::pend(crate::pac::Interrupt::OS_EVENT);

        AuthenticatorInfo {
            versions,
            extensions: Some(extensions),
            aaguid: ByteBuf::from_slice(AAGUID).unwrap(),
            options: Some(options),
            max_msg_size: Some(MESSAGE_SIZE),
            ..AuthenticatorInfo::default()
        }
    }

    fn reset(&mut self) -> Result<()> {
        self.reset_master_secret().expect("reset master secret failed, oh noe");
        todo!("reset");
    }

    fn get_assertions(&mut self, params: &GetAssertionParameters) -> Result<AssertionResponses> {
        todo!("get_assertions");
    }

    fn make_credential(&mut self, params: &MakeCredentialParameters) -> Result<AttestationObject> {
        todo!("make_credential");
    }
}
