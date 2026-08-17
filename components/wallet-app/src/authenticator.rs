use crate::{command::Command, key_derivation::derive_keypair, state::State, WALLET_AID};
use iso7816::{Data, Status};
use trussed_core::syscall;
use trussed_core::InterruptFlag;

#[cfg(feature = "dispatch")]
use crate::dispatch::{
    app::App, types::Error, types::Message, types::ResponseMessage, types::DEFAULT_MESSAGE_SIZE,
};

/// Solana off-chain message signing domain (`\xff` + "solana offchain").
const OFFCHAIN_DOMAIN: &[u8] = b"\xffsolana offchain";

/// State for a sign that is waiting on the runner-driven, non-blocking
/// user-presence signal. The message bytes live in `Authenticator::sign_buf`; the
/// path bytes live in `Authenticator::sign_path` (both stay put because the
/// dispatch won't take a new request while a sign is pending).
struct PendingSign {
    chain: crate::chain::Chain,
}

/// If `msg` is a Solana off-chain message (`\xffsolana offchain` + version +
/// format + u16-LE length + text), return the bare text; otherwise return `msg`
/// unchanged. Lets the host (dApp) verify the signature over the SIWS message
/// itself rather than the off-chain wrapper.
fn strip_offchain_header(msg: &[u8]) -> &[u8] {
    let header = OFFCHAIN_DOMAIN.len() + 4; // domain + version + format + u16 len
    if msg.len() >= header && msg.starts_with(OFFCHAIN_DOMAIN) {
        let text_len = u16::from_le_bytes([msg[header - 2], msg[header - 1]]) as usize;
        if header + text_len == msg.len() {
            return &msg[header..];
        }
    }
    msg
}

/// Wallet authenticator
pub struct Authenticator<T> {
    trussed: T,
    state: State,
    // Runner-owned cancel flag for the user-presence wait. We can't get it
    // through `T: Client` — the `PollClient::interrupt` default is None and
    // `ClientImplementation`'s real getter is an inherent method invisible
    // behind the trait bound — so the runner hands it in at construction.
    interrupt: Option<&'static InterruptFlag>,
    // Active chain (RAM only, resets to Solana each boot). Set explicitly via
    // SET_CHAIN; gates the Ethereum getAppConfiguration probe so the host
    // detects the chosen app. `true` = Ethereum.
    active_chain_eth: bool,
    // Shared staging buffer (NOT eth-only): accumulates the chunked Ethereum
    // signTransaction RLP, AND holds the Solana message while a sign is pending
    // (`begin_sign` copies it here so it outlives the user-presence wait). 4 KB
    // headroom for a future large Solana tx.
    sign_buf: heapless::Vec<u8, 4096>,
    sign_path: heapless::Vec<u8, 48>,
    /// Total expected tx length; 0 = no signTransaction in progress.
    eth_expected_len: usize,
    // A sign waiting on user presence; `None` = no sign pending. While `Some`,
    // the dispatch polls it instead of taking new requests.
    pending: Option<PendingSign>,
}

impl<T> Authenticator<T>
where
    T: trussed::Client,
{
    pub fn new(trussed: T) -> Self {
        Self::with_interrupt(trussed, None)
    }

    pub fn with_interrupt(trussed: T, interrupt: Option<&'static InterruptFlag>) -> Self {
        info_now!("Wallet app initialized");
        Self {
            trussed,
            state: State::default(),
            interrupt,
            active_chain_eth: false,
            sign_buf: heapless::Vec::new(),
            sign_path: heapless::Vec::new(),
            eth_expected_len: 0,
            pending: None,
        }
    }

    /// Handle an APDU command (simple ISO7816 format, like OATH).
    ///
    /// This is the SYNCHRONOUS entry point: a signMessage that begins a pending
    /// sign is pumped to completion here before returning, so direct callers
    /// (the in-process test transport) keep the usual blocking APDU contract.
    /// The on-device dispatch does NOT use this path for signing — it goes
    /// through `App::call` + `App::poll` (`dispatch_command` + `poll_sign`),
    /// which never blocks.
    pub fn respond<const C: usize, const R: usize>(
        &mut self,
        command: &iso7816::Command<C>,
        reply: &mut Data<R>,
    ) -> Result<(), Status> {
        let result = self.dispatch_command(command, reply);

        // If a sign just went pending, drive it to completion synchronously.
        // On device the runner's idle loop fills the consent result; here (the
        // in-process sim pump) nothing else will, so resolve the result ONCE
        // from the sim's own user-presence decision (trussed `check_user_presence`,
        // driven by the test's approve/deny) and then pump `poll_sign`.
        if self.pending.is_some() {
            const TIMEOUT_MS: u32 = 30000;
            let granted = syscall!(self.trussed.confirm_user_present(TIMEOUT_MS))
                .result
                .is_ok();
            crate::consent::set_up_result(if granted {
                crate::consent::GRANTED
            } else {
                crate::consent::TIMED_OUT
            });
            loop {
                if let Some(r) = self.poll_sign(reply) {
                    break r;
                }
            }
        } else {
            result
        }
    }

    /// Parse + dispatch one APDU. Does NOT pump a pending sign — a signMessage
    /// returns with `self.pending = Some` and an empty reply. Used by both the
    /// blocking `respond` and the non-blocking `App::call`.
    fn dispatch_command<const C: usize, const R: usize>(
        &mut self,
        command: &iso7816::Command<C>,
        reply: &mut Data<R>,
    ) -> Result<(), Status> {
        // Check if this is a SELECT command (standard ISO7816)
        if command.instruction() == iso7816::Instruction::Select {
            // Return AID on SELECT - this is required for the app to be recognized
            let aid = WALLET_AID;
            reply
                .extend_from_slice(aid)
                .map_err(|_| Status::NotEnoughMemory)?;
            return Ok(());
        }

        // Parse the command (standard ISO7816 format)
        let cmd = command.try_into()?;

        match cmd {
            Command::GetAppConfiguration => self.get_app_configuration(reply),
            Command::GetPubkey(get_pubkey) => self.get_pubkey(get_pubkey, reply),
            Command::SignMessage { p2, chunk } => self.sign_message(p2, chunk, false, reply),
            Command::SignOffchainMessage { p2, chunk } => self.sign_message(p2, chunk, true, reply),
            Command::Reset => self.reset(reply),
            Command::Keygen { export } => self.keygen(export, reply),
            Command::SetSeed(seed_bytes) => self.set_seed(seed_bytes, reply),
            Command::SetPrivateKey(key_bytes) => self.set_private_key(key_bytes, reply),
            Command::GetSecretType => self.get_secret_type(reply),
            Command::EthGetAddress(get_address) => self.eth_get_address(get_address, reply),
            Command::EthGetAppConfiguration => self.eth_get_app_configuration(reply),
            Command::EthSignTransaction { first, chunk } => {
                self.eth_sign_transaction(first, chunk, reply)
            }
            Command::EthSignPersonalMessage { first, chunk } => {
                self.eth_sign_personal_message(first, chunk, reply)
            }
            Command::SetChain { eth } => self.set_chain(eth, reply),
        }
    }

    /// Ledger Ethereum app signPersonalMessage (`e0 08`, EIP-191). Accumulates
    /// the chunked message, then signs the EIP-191 digest and returns `[v][r][s]`
    /// with `v = 27 + recovery_id`. Intermediate chunks return an empty 0x9000.
    fn eth_sign_personal_message<const R: usize>(
        &mut self,
        first: bool,
        chunk: &[u8],
        reply: &mut Data<R>,
    ) -> Result<(), Status> {
        if first {
            // [pathLen][path components][msgLen(4B BE)][message start].
            let path_components = *chunk.first().ok_or(Status::WrongLength)? as usize;
            let path_len = 1 + path_components * 4;
            let len_bytes = chunk
                .get(path_len..path_len + 4)
                .ok_or(Status::WrongLength)?;
            let msg_len =
                u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]])
                    as usize;
            let msg = chunk.get(path_len + 4..).ok_or(Status::WrongLength)?;

            self.sign_buf.clear();
            self.sign_path.clear();
            self.sign_path
                .extend_from_slice(&chunk[..path_len])
                .map_err(|_| Status::WrongLength)?;
            self.sign_buf
                .extend_from_slice(msg)
                .map_err(|_| Status::NotEnoughMemory)?;
            self.eth_expected_len = msg_len;
        } else {
            if self.eth_expected_len == 0 {
                return Err(Status::ConditionsOfUseNotSatisfied);
            }
            self.sign_buf
                .extend_from_slice(chunk)
                .map_err(|_| Status::NotEnoughMemory)?;
        }

        if self.sign_buf.len() < self.eth_expected_len {
            return Ok(());
        }

        let secret_type = self
            .state
            .persistent(&mut self.trussed, |_trussed, state| state.secret_type);
        if secret_type == crate::state::SecretType::Empty.to_byte() {
            self.eth_expected_len = 0;
            return Err(Status::NotFound);
        }
        self.require_user_presence()?;

        let secret_state = self
            .state
            .persistent(&mut self.trussed, |_trussed, state| state.clone());
        let path = crate::derivation_path::DerivationPath::parse(&self.sign_path)?;
        let priv_key = crate::key_derivation::derive_secp256k1_priv(&secret_state, &path)
            .ok_or(Status::NotFound)?;

        let digest = crate::eth::personal_message_hash(&self.sign_buf);
        let sig = crate::signing::sign_prehash(&priv_key, &digest).ok_or(Status::NotFound)?;
        let v = 27 + sig[64]; // ETHEREUM_SIGNATURE_V_BASE + recovery id
        self.eth_expected_len = 0;

        reply.push(v).map_err(|_| Status::NotEnoughMemory)?;
        reply
            .extend_from_slice(&sig[..64])
            .map_err(|_| Status::NotEnoughMemory)?;
        Ok(())
    }

    /// Ledger Ethereum app signTransaction (`e0 04` with data). Accumulates the
    /// chunked RLP, and once the whole tx is present, blind-signs
    /// `keccak256(tx)` and returns `[v][r][s]`. Intermediate chunks return an
    /// empty 0x9000.
    fn eth_sign_transaction<const R: usize>(
        &mut self,
        first: bool,
        chunk: &[u8],
        reply: &mut Data<R>,
    ) -> Result<(), Status> {
        if first {
            // [pathLen][path components][RLP tx start].
            let path_components = *chunk.first().ok_or(Status::WrongLength)? as usize;
            let path_len = 1 + path_components * 4;
            let rlp = chunk.get(path_len..).ok_or(Status::WrongLength)?;

            self.sign_buf.clear();
            self.sign_path.clear();
            self.sign_path
                .extend_from_slice(&chunk[..path_len])
                .map_err(|_| Status::WrongLength)?;
            self.sign_buf
                .extend_from_slice(rlp)
                .map_err(|_| Status::NotEnoughMemory)?;
            self.eth_expected_len =
                crate::eth::tx_total_len(rlp).ok_or(Status::IncorrectDataParameter)?;
        } else {
            if self.eth_expected_len == 0 {
                return Err(Status::ConditionsOfUseNotSatisfied);
            }
            self.sign_buf
                .extend_from_slice(chunk)
                .map_err(|_| Status::NotEnoughMemory)?;
        }

        // Not all chunks received yet: ack with an empty 0x9000.
        if self.sign_buf.len() < self.eth_expected_len {
            return Ok(());
        }

        // Whole tx present — require presence, then sign.
        let secret_type = self
            .state
            .persistent(&mut self.trussed, |_trussed, state| state.secret_type);
        if secret_type == crate::state::SecretType::Empty.to_byte() {
            self.eth_expected_len = 0;
            return Err(Status::NotFound);
        }
        self.require_user_presence()?;

        let secret_state = self
            .state
            .persistent(&mut self.trussed, |_trussed, state| state.clone());
        let path = crate::derivation_path::DerivationPath::parse(&self.sign_path)?;
        let priv_key = crate::key_derivation::derive_secp256k1_priv(&secret_state, &path)
            .ok_or(Status::NotFound)?;

        // eth_sign hashes keccak256(message) and returns r||s||recid.
        let sig = crate::signing::eth_sign(&priv_key, &self.sign_buf).ok_or(Status::NotFound)?;
        let recid = sig[64];

        // v: typed tx → recovery id (0/1); legacy EIP-155 → chainId*2+35+recid.
        let v = if crate::eth::is_typed_tx(&self.sign_buf) {
            recid
        } else {
            let chain_id = crate::eth::legacy_chain_id(&self.sign_buf).unwrap_or(0);
            (chain_id * 2 + 35 + recid as u64) as u8
        };
        self.eth_expected_len = 0;

        reply.push(v).map_err(|_| Status::NotEnoughMemory)?;
        reply
            .extend_from_slice(&sig[..64])
            .map_err(|_| Status::NotEnoughMemory)?;
        Ok(())
    }

    /// Ledger Ethereum app getAppConfiguration (`e0 06` with no data), the
    /// command Phantom/MetaMask use to detect which app is "open".
    ///
    /// Phantom's app-detection probe is byte-identical for Solana and Ethereum,
    /// runs twice and keeps the last result, so it can't be auto-detected. The
    /// device's active chain is therefore set explicitly (SET_CHAIN; default
    /// Solana): only when it is Ethereum does this return the eth config — so
    /// Phantom detects Ethereum. While the active chain is Solana this errors,
    /// so Phantom falls through to the Solana `e0 04` config. MetaMask skips this
    /// probe (it sends `e0 02` getAddress directly), so it is unaffected.
    fn eth_get_app_configuration<const R: usize>(&self, reply: &mut Data<R>) -> Result<(), Status> {
        if self.active_chain_eth {
            reply
                .extend_from_slice(&[0x00, 0x01, 0x07, 0x02])
                .map_err(|_| Status::NotEnoughMemory)
        } else {
            // The same 0x6700 the Solana app returns for an empty signMessage.
            Err(Status::WrongLength)
        }
    }

    /// Set the active chain (`0x16`): P1 = 0x01 → Ethereum, else Solana. RAM
    /// only — resets to Solana on the next power-up.
    fn set_chain<const R: usize>(&mut self, eth: bool, _reply: &mut Data<R>) -> Result<(), Status> {
        self.active_chain_eth = eth;
        Ok(())
    }

    /// Ledger Ethereum app getAddress: derive secp256k1 at `path` and return
    /// `[65][uncompressed pubkey][40][EIP-55 ascii address][chaincode(32) if P2&1]`.
    fn eth_get_address<const R: usize>(
        &mut self,
        get_address: crate::command::EthGetAddress,
        reply: &mut Data<R>,
    ) -> Result<(), Status> {
        let secret_state = self
            .state
            .persistent(&mut self.trussed, |_trussed, state| state.clone());
        if secret_state.secret_type == crate::state::SecretType::Empty.to_byte() {
            return Err(Status::NotFound);
        }

        // P1 = confirm asks for an on-device approval before revealing the key.
        if get_address.display {
            self.require_user_presence()?;
        }

        let (priv_key, chain_code) = crate::key_derivation::derive_secp256k1_with_chaincode(
            &secret_state,
            &get_address.derivation_path,
        )
        .ok_or(Status::NotFound)?;
        let pubkey = crate::key_derivation::secp256k1_pubkey_uncompressed(&priv_key)
            .ok_or(Status::NotFound)?;
        let address = crate::eth::address_ascii(&pubkey);

        reply
            .push(pubkey.len() as u8)
            .map_err(|_| Status::NotEnoughMemory)?;
        reply
            .extend_from_slice(&pubkey)
            .map_err(|_| Status::NotEnoughMemory)?;
        reply
            .push(address.len() as u8)
            .map_err(|_| Status::NotEnoughMemory)?;
        reply
            .extend_from_slice(&address)
            .map_err(|_| Status::NotEnoughMemory)?;
        if get_address.chaincode {
            reply
                .extend_from_slice(&chain_code)
                .map_err(|_| Status::NotEnoughMemory)?;
        }
        Ok(())
    }

    /// Get application configuration (get_configuration_vector)
    fn get_app_configuration<const R: usize>(&self, reply: &mut Data<R>) -> Result<(), Status> {
        // Return configuration vector as 5-byte binary format
        // Format:
        // - Byte 0: enable_blind_signing (0x00 = disabled, 0x01 = enabled)
        // - Byte 1: pubkey_display (0x00 = long, 0x01 = short)
        // - Byte 2: major version (1)
        // - Byte 3: minor version (7)
        // - Byte 4: patch version (2)
        // Version: 1.7.2

        let config_response: &[u8] = &[
            0x01, // enable_blind_signing: enabled
            0x00, // pubkey_display: long
            0x01, // major version: 1
            0x07, // minor version: 7
            0x02, // patch version: 2
        ];

        reply
            .extend_from_slice(config_response)
            .map_err(|_| Status::NotEnoughMemory)?;

        Ok(())
    }

    /// Get public key for derivation path
    fn get_pubkey<const R: usize>(
        &mut self,
        get_pubkey: crate::command::GetPubkey,
        reply: &mut Data<R>,
    ) -> Result<(), Status> {
        let secret_state = self
            .state
            .persistent(&mut self.trussed, |_trussed, state| state.clone());

        if secret_state.secret_type == crate::state::SecretType::Empty.to_byte() {
            return Err(Status::NotFound);
        }

        let path = get_pubkey.derivation_path;
        match crate::chain::Chain::from_path(&path) {
            // Solana: 32-byte Ed25519 public key.
            crate::chain::Chain::Solana => {
                let keypair = derive_keypair(&secret_state, path);
                reply
                    .extend_from_slice(keypair.public.as_bytes())
                    .map_err(|_| Status::NotEnoughMemory)?;
            }
            // Ethereum: 65-byte uncompressed secp256k1 key (host hashes
            // X||Y with Keccak-256 for the address).
            crate::chain::Chain::Ethereum => {
                let priv_key = crate::key_derivation::derive_secp256k1_priv(&secret_state, &path)
                    .ok_or(Status::NotFound)?;
                let pubkey = crate::key_derivation::secp256k1_pubkey_uncompressed(&priv_key)
                    .ok_or(Status::NotFound)?;
                reply
                    .extend_from_slice(&pubkey)
                    .map_err(|_| Status::NotEnoughMemory)?;
            }
        }

        Ok(())
    }

    /// Sign a message
    /// Block until the user approves the operation via the runner's presence
    /// check. Backed by trussed `confirm_user_present`, so it is satisfied by a
    /// physical button on a buttons build, the JTAG `UP_CONTROL` override under
    /// `test-up-control`, or auto-approved on a no-buttons build. Returns
    /// `ConditionsOfUseNotSatisfied` on timeout or reject.
    fn require_user_presence(&mut self) -> Result<(), Status> {
        // Move the cancel flag Idle → Working so a runner-side reject button can
        // flip it Working → Interrupted and break trussed's consent loop.
        if let Some(i) = self.interrupt {
            i.set_working();
        }
        const TIMEOUT_MS: u32 = 30000;
        let result = syscall!(self.trussed.confirm_user_present(TIMEOUT_MS)).result;
        if let Some(i) = self.interrupt {
            i.set_idle();
        }
        if result.is_err() {
            return Err(Status::ConditionsOfUseNotSatisfied);
        }
        Ok(())
    }

    /// Ledger Solana signMessage (0x06) / signOffchainMessage (0x07). The host
    /// may chunk the payload across APDUs via P2: the first chunk (`!EXTEND`)
    /// carries `[numPaths][path]` then message bytes, continuations (`EXTEND`)
    /// carry raw message bytes, and signing happens only once `MORE` is clear.
    /// An unchunked APDU (P2 = 0) signs directly from the request — large
    /// messages aren't bounded by the buffer; chunked sends accumulate into the
    /// shared `sign_buf` buffer. Intermediate chunks return an empty 0x9000.
    fn sign_message<const R: usize>(
        &mut self,
        p2: u8,
        chunk: &[u8],
        offchain: bool,
        reply: &mut Data<R>,
    ) -> Result<(), Status> {
        const P2_EXTEND: u8 = 0x01;
        const P2_MORE: u8 = 0x02;
        let extend = p2 & P2_EXTEND != 0;
        let more = p2 & P2_MORE != 0;

        // Unchunked single APDU: parse and begin the sign.
        // `begin_sign` copies path + message into the owned buffers, so the
        // borrows of `chunk` don't outlive this call.
        if !extend && !more {
            let sm = crate::command::parse_sign_message(chunk)?;
            return self.begin_sign(sm.derivation_path, sm.message, offchain, reply);
        }

        // Chunked: accumulate path + message into the shared sign buffer.
        if !extend {
            let sm = crate::command::parse_sign_message(chunk)?;
            let path_len = 1 + sm.derivation_path.depth as usize * 4;
            self.sign_buf.clear();
            self.sign_path.clear();
            self.sign_path
                .extend_from_slice(&chunk[1..1 + path_len])
                .map_err(|_| Status::NotEnoughMemory)?;
            self.sign_buf
                .extend_from_slice(sm.message)
                .map_err(|_| Status::NotEnoughMemory)?;
        } else {
            // Continuation must follow a first chunk that set the path.
            if self.sign_path.is_empty() {
                return Err(Status::ConditionsOfUseNotSatisfied);
            }
            self.sign_buf
                .extend_from_slice(chunk)
                .map_err(|_| Status::NotEnoughMemory)?;
        }

        // More chunks coming — acknowledge with an empty 0x9000.
        if more {
            return Ok(());
        }

        // Last chunk: take the buffers out of `self` (also resets them) so the
        // parsed path doesn't borrow `self` across `begin_sign`. `begin_sign`
        // re-copies into the owned buffers, so they end up populated again.
        let path_bytes = core::mem::take(&mut self.sign_path);
        let message = core::mem::take(&mut self.sign_buf);
        let path = crate::derivation_path::DerivationPath::parse(&path_bytes)?;
        self.begin_sign(path, &message, offchain, reply)
    }

    /// Begin a sign of `message` under `path`. Stages the message
    /// (into `sign_buf`) and path (into `sign_path`), publishes the NDEF preview
    /// for a Solana tx, and arms the runner-driven user-presence signal WITHOUT
    /// blocking (`consent::request_up`). Leaves `self.pending = Some`, so the
    /// dispatch does not respond until `poll_sign` finishes. Synchronous errors
    /// (no secret, buffer too small) return immediately and leave no pending.
    fn begin_sign<const R: usize>(
        &mut self,
        path: crate::derivation_path::DerivationPath,
        message: &[u8],
        offchain: bool,
        _reply: &mut Data<R>,
    ) -> Result<(), Status> {
        let secret_type = self
            .state
            .persistent(&mut self.trussed, |_trussed, state| state.secret_type);
        if secret_type == crate::state::SecretType::Empty.to_byte() {
            return Err(Status::NotFound);
        }

        // SIWS sign-in (0x07) arrives wrapped in the `\xffsolana offchain`
        // envelope; sign only the bare message text so dApp verifiers that check
        // the signature over the unwrapped SIWS message accept it.
        let message = if offchain {
            strip_offchain_header(message)
        } else {
            message
        };

        let chain = crate::chain::Chain::from_path(&path);

        // Stage path + message into the owned buffers so they survive the wait
        // (the `message`/`path` borrows may point at the transient request).
        let mut path_bytes = heapless::Vec::<u8, 48>::new();
        path_bytes
            .push(path.depth)
            .map_err(|_| Status::NotEnoughMemory)?;
        path_bytes
            .extend_from_slice(path.components)
            .map_err(|_| Status::NotEnoughMemory)?;
        self.sign_buf.clear();
        self.sign_buf
            .extend_from_slice(message)
            .map_err(|_| Status::NotEnoughMemory)?;
        self.sign_path = path_bytes;

        if chain == crate::chain::Chain::Solana && !offchain {
            crate::nfc::set_signer_override(&self.sign_buf);
        }

        // Begin waiting for user presence. The runner computes the result in its
        // idle loop (non-blocking); we just poll it via `poll_sign`.
        crate::consent::request_up();

        self.pending = Some(PendingSign { chain });
        Ok(())
    }

    /// Finish a granted sign: derive the key, sign the staged message (read from
    /// `sign_buf`, path from `sign_path`) and write the signature into `reply`.
    fn finish_sign<const R: usize>(
        &mut self,
        chain: crate::chain::Chain,
        reply: &mut Data<R>,
    ) -> Result<(), Status> {
        let path_bytes = core::mem::take(&mut self.sign_path);
        let message = core::mem::take(&mut self.sign_buf);
        let path = crate::derivation_path::DerivationPath::parse(&path_bytes)?;

        let secret_state = self
            .state
            .persistent(&mut self.trussed, |_trussed, state| state.clone());
        match chain {
            // Solana: Ed25519 over the raw message → 64-byte signature.
            crate::chain::Chain::Solana => {
                let keypair = derive_keypair(&secret_state, path);
                let signature = keypair.sign(&message);
                reply
                    .extend_from_slice(&signature.to_bytes())
                    .map_err(|_| Status::NotEnoughMemory)?;
            }
            // Ethereum: Keccak-256(message) → recoverable ECDSA, 65-byte r||s||v.
            crate::chain::Chain::Ethereum => {
                let priv_key = crate::key_derivation::derive_secp256k1_priv(&secret_state, &path)
                    .ok_or(Status::NotFound)?;
                let signature =
                    crate::signing::eth_sign(&priv_key, &message).ok_or(Status::NotFound)?;
                reply
                    .extend_from_slice(&signature)
                    .map_err(|_| Status::NotEnoughMemory)?;
            }
        }

        Ok(())
    }

    /// Clear the user-presence wait state (consent signal + NDEF override).
    /// Called once the sign resolves either way.
    fn teardown_sign(&mut self) {
        crate::consent::clear_up();
        crate::nfc::clear();
    }

    /// Advance a pending sign without blocking. Reads the runner-published
    /// consent result (`consent::up_result`): `WAITING` → yield (`None`) so the
    /// idle loop runs NDEF; `GRANTED` → tear down + finish the sign; `TIMED_OUT`
    /// → tear down + give up. Returns `Some` once the sign resolves (granted →
    /// signature in `reply`; timed out → `ConditionsOfUseNotSatisfied`). Always
    /// leaves `self.pending = None` when it returns `Some`.
    fn poll_sign<const R: usize>(&mut self, reply: &mut Data<R>) -> Option<Result<(), Status>> {
        let p = self.pending.take().expect("poll_sign with no pending");

        match crate::consent::up_result() {
            crate::consent::GRANTED => {
                self.teardown_sign();
                Some(self.finish_sign(p.chain, reply))
            }
            crate::consent::TIMED_OUT => {
                self.teardown_sign();
                Some(Err(Status::ConditionsOfUseNotSatisfied))
            }
            // Still waiting — yield so the idle loop services NFC.
            _ => {
                self.pending = Some(p);
                None
            }
        }
    }

    /// Reset secret to empty
    fn reset<const R: usize>(&mut self, _reply: &mut Data<R>) -> Result<(), Status> {
        // Wiping the seed is destructive — gate it on user presence.
        self.require_user_presence()?;
        self.state.persistent(&mut self.trussed, |_trussed, state| {
            state.secret_type = crate::state::SecretType::Empty.to_byte();
            state.secret_bytes = [0u8; 32];
        });
        Ok(())
    }

    /// Generate a new seed
    fn keygen<const R: usize>(&mut self, export: bool, reply: &mut Data<R>) -> Result<(), Status> {
        // Generating (and overwriting) the seed is seed-affecting — gate on UP.
        self.require_user_presence()?;
        let random_bytes = syscall!(self.trussed.random_bytes(32)).bytes;
        let mut seed_bytes = [0u8; 32];
        seed_bytes.copy_from_slice(&random_bytes.as_slice()[..32]);

        self.state.persistent(&mut self.trussed, |_trussed, state| {
            state.secret_type = if export { 0x03 } else { 0x04 };
            state.secret_bytes = seed_bytes;
        });

        if export {
            reply
                .extend_from_slice(&seed_bytes)
                .map_err(|_| Status::NotEnoughMemory)?;
        }

        Ok(())
    }

    /// Set a new seed
    fn set_seed<const R: usize>(
        &mut self,
        seed_bytes: &[u8],
        _reply: &mut Data<R>,
    ) -> Result<(), Status> {
        if seed_bytes.len() != 32 {
            return Err(Status::WrongLength);
        }
        // Importing (and overwriting) the seed is seed-affecting — gate on UP.
        self.require_user_presence()?;

        let mut seed = [0u8; 32];
        seed.copy_from_slice(seed_bytes);

        self.state.persistent(&mut self.trussed, |_trussed, state| {
            state.secret_type = 0x02; // ImportedSeed
            state.secret_bytes = seed;
        });

        Ok(())
    }

    /// Set a new private key
    fn set_private_key<const R: usize>(
        &mut self,
        key_bytes: &[u8],
        _reply: &mut Data<R>,
    ) -> Result<(), Status> {
        if key_bytes.len() != 32 {
            return Err(Status::WrongLength);
        }
        // Importing (and overwriting) the private key is seed-affecting — gate on UP.
        self.require_user_presence()?;

        let mut key = [0u8; 32];
        key.copy_from_slice(key_bytes);

        self.state.persistent(&mut self.trussed, |_trussed, state| {
            state.secret_type = 0x01; // PrivateKey
            state.secret_bytes = key;
        });

        Ok(())
    }

    /// Get secret type
    fn get_secret_type<const R: usize>(&mut self, reply: &mut Data<R>) -> Result<(), Status> {
        let secret_type = self
            .state
            .persistent(&mut self.trussed, |_trussed, state| state.secret_type);

        reply
            .push(secret_type)
            .map_err(|_| Status::NotEnoughMemory)?;

        Ok(())
    }
}

impl<T> iso7816::App for Authenticator<T> {
    fn aid(&self) -> iso7816::Aid {
        iso7816::Aid::new(WALLET_AID)
    }
}

#[cfg(feature = "dispatch")]
impl<T> App for Authenticator<T>
where
    T: trussed::Client,
{
    fn call(&mut self, request: &Message, response: &mut ResponseMessage) -> Result<(), Error> {
        // Parse request as ISO7816 command (transport already unwrapped by the
        // usbd layer). Capacity = the full request buffer so a large tx fits.
        let iso_command: iso7816::Command<DEFAULT_MESSAGE_SIZE> =
            match iso7816::Command::<DEFAULT_MESSAGE_SIZE>::try_from(request.as_slice()) {
                Ok(cmd) => cmd,
                Err(_) => return Err(Error::InvalidLength),
            };

        // Dispatch without pumping a pending sign: if this is a signMessage it
        // returns with `self.pending = Some` and the dispatch drives `poll`
        // (the non-blocking path). Everything else resolves synchronously.
        let mut reply_data = Data::<256>::new();
        let result = self.dispatch_command(&iso_command, &mut reply_data);
        encode_apdu_response(result, &reply_data, response)
    }

    fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    fn poll(&mut self, response: &mut ResponseMessage) -> Option<Result<(), Error>> {
        let mut reply_data = Data::<256>::new();
        let result = self.poll_sign(&mut reply_data)?;
        // Finished — encode exactly like `call` does.
        Some(encode_apdu_response(result, &reply_data, response))
    }
}

/// Encode an APDU `respond`/`poll_sign` outcome into the HID response: on `Ok`,
/// `reply_data` bytes followed by `9000`; on `Err`, the 2-byte status word.
/// Shared by `call` (synchronous) and `poll` (finished sign) so a completed
/// sign is framed identically.
#[cfg(feature = "dispatch")]
fn encode_apdu_response<const R: usize>(
    result: Result<(), Status>,
    reply_data: &Data<R>,
    response: &mut ResponseMessage,
) -> Result<(), Error> {
    match result {
        Ok(_) => {
            response
                .extend_from_slice(reply_data.as_slice())
                .map_err(|_| Error::InvalidLength)?;
            // Success status (0x9000).
            response.push(0x90).map_err(|_| Error::InvalidLength)?;
            response.push(0x00).map_err(|_| Error::InvalidLength)?;
        }
        Err(status) => {
            let status_u16 = u16::from(status);
            response
                .push((status_u16 >> 8) as u8)
                .map_err(|_| Error::InvalidLength)?;
            response
                .push(status_u16 as u8)
                .map_err(|_| Error::InvalidLength)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // Note: Tests for get_pubkey and sign_message are disabled because they require
    // a real trussed client with filesystem support. These are tested in integration tests.
    // The key_derivation tests in key_derivation.rs validate the core logic.

    #[cfg(all(test, feature = "usbd"))]
    #[test]
    fn test_transport_wrapped_get_app_configuration() {
        let apdu_bytes: [u8; 64] = [
            0x01, 0x01, 0x05, 0x00, 0x00, 0x00, 0x05, 0xE0, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        use crate::usbd::pipe::unwrap_transport;
        let command: iso7816::Command<300> = unwrap_transport(&apdu_bytes).unwrap();

        // Transport unwrap yields the inner ISO-7816 GET_APP_CONFIGURATION.
        // (Driving `respond()` needs a real trussed client; that path is
        // covered by the pc integration tests in runners/pc/tests/wallet.rs.)
        assert_eq!(command.class().into_inner(), 0xE0);
        assert_eq!(u8::from(command.instruction()), 0x04);
        assert_eq!(command.p1, 0x00);
        assert_eq!(command.p2, 0x00);
        assert_eq!(command.data().len(), 0);
    }
}
