use core::convert::From;
use core::convert::TryFrom;
use ctap_types::{
    authenticator::{Request, Error as AuthenticatorError},
    operation::Operation,
    serde::{cbor_deserialize, error::Error as SerdeError},
};

use crate::logger::{info, blocking};

pub enum CtapMappingError {
    InvalidCommand(u8),
    ParsingError(SerdeError),
}

impl From<CtapMappingError> for AuthenticatorError {
    fn from(mapping_error: CtapMappingError) -> AuthenticatorError {
        match mapping_error {
            CtapMappingError::InvalidCommand(_cmd) => {
                AuthenticatorError::InvalidCommand
            }
            CtapMappingError::ParsingError(cbor_error) => {
                match cbor_error {
                    SerdeError::SerdeMissingField => AuthenticatorError::MissingParameter,
                    _ => AuthenticatorError::InvalidCbor
                }
            }
        }

    }
}

pub fn parse_cbor(data: &[u8]) -> core::result::Result<Request, CtapMappingError> {
    // let data = &buffer[..request.length as usize];
    // blocking::info!("data: {:?}", data).ok();

    if data.len() < 1 {
        return Err(CtapMappingError::ParsingError(SerdeError::DeserializeUnexpectedEnd));
    }

    let operation_u8: u8 = data[0];

    let operation = match Operation::try_from(operation_u8) {
        Ok(operation) => {
            operation
        },
        _ => {
            return Err(CtapMappingError::InvalidCommand(operation_u8));
        }
    };

    // use ctap_types::ctap2::*;
    use ctap_types::authenticator::*;

    match operation {
        Operation::MakeCredential => {
            info!("authenticatorMakeCredential").ok();
            match cbor_deserialize(&data[1..]) {
                Ok(params) => {
                    Ok(Request::Ctap2(ctap2::Request::MakeCredential(params)))
                },
                Err(error) => {
                    Err(CtapMappingError::ParsingError(error))
                }
            }
            // TODO: ensure earlier that RPC send queue is empty
        }

        Operation::GetAssertion => {
            info!("authenticatorGetAssertion").ok();

            match cbor_deserialize(&data[1..]) {
                Ok(params) => {
                    Ok(Request::Ctap2(ctap2::Request::GetAssertion(params)))
                },
                Err(error) => {
                    Err(CtapMappingError::ParsingError(error))
                }
            }
            // TODO: ensure earlier that RPC send queue is empty
        }

        Operation::GetNextAssertion => {
            info!("authenticatorGetNextAssertion").ok();

            // TODO: ensure earlier that RPC send queue is empty
            Ok(Request::Ctap2(ctap2::Request::GetNextAssertion))
        }

        Operation::CredentialManagement => {
            info!("authenticatorCredentialManagement").ok();

            match cbor_deserialize(&data[1..]) {
                Ok(params) => {
                    Ok(Request::Ctap2(ctap2::Request::CredentialManagement(params)))
                },
                Err(error) => {
                    Err(CtapMappingError::ParsingError(error))
                }
            }
            // TODO: ensure earlier that RPC send queue is empty
        }

        Operation::Reset => {
            info!("authenticatorReset").ok();

            // TODO: ensure earlier that RPC send queue is empty
            Ok(Request::Ctap2(ctap2::Request::Reset))
        }

        Operation::GetInfo => {
            info!("authenticatorGetInfo").ok();
            // TODO: ensure earlier that RPC send queue is empty
            Ok(Request::Ctap2(ctap2::Request::GetInfo))
        }

        Operation::ClientPin => {
            info!("authenticatorClientPin").ok();
            match cbor_deserialize(&data[1..])
            {
                Ok(params) => {

                    Ok(Request::Ctap2(ctap2::Request::ClientPin(params)))
                },
                Err(error) => {

                    Err(CtapMappingError::ParsingError(error))
                }
            }
            // TODO: ensure earlier that RPC send queue is empty
        }

        Operation::Vendor(vendor_operation) => {
            info!("authenticatorVendor({:?})", &vendor_operation).ok();

            let vo_u8: u8 = vendor_operation.into();
            if vo_u8 == 0x41 {
                // copy-pasta for now
                match cbor_deserialize(&data[1..])
                {
                    Ok(params) => {
                        Ok(Request::Ctap2(ctap2::Request::CredentialManagement(params)))
                    },
                    Err(error) => {
                        Err(CtapMappingError::ParsingError(error))
                    }
                }
                // TODO: ensure earlier that RPC send queue is empty

            } else {
                // TODO: ensure earlier that RPC send queue is empty
                Ok(Request::Ctap2(ctap2::Request::Vendor(vendor_operation)))
            }
        }
        _ => {
            Err(CtapMappingError::InvalidCommand(operation_u8))
        }
    }
}