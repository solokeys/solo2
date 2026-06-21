# FIDO2 with a Solo key

> **Works on: Secure + Hacker.** Core FIDO2/WebAuthn.

Solo 2 is a FIDO2/CTAP2 authenticator. Credentials are **ES256 (ECDSA P-256, alg `-7`)** or **EdDSA (Ed25519, alg `-8`)**; the **`hmac-secret`** extension provides a per-credential symmetric secret used by tools like [age](AGE.md) and LUKS.

## Passkeys in the browser
Register/sign in at <https://webauthn.io> (or any FIDO2 site: GitHub, Google, …). Resident credential + PIN + touch = a passkey.

## With the `solo2` CLI
```bash
cargo install solo2          # provides the `solo2` binary
solo2 ls                     # devices + firmware, e.g. "Solo 2 … (CTAP+PCSC, firmware 2:…)"
solo2 app fido init          # FIDO init response: major/minor, can_wink/can_cbor/can_msg
solo2 app fido wink          # blink the key to identify it
```
The CLI manages the device; credential **create/assert** is done by a FIDO2 client (browser, or the snippets below).

## Programmatic: make a credential with each algorithm
With [python-fido2](https://github.com/Yubico/python-fido2). Touch the Solo at each `make_credential`.
```python
import hashlib
from fido2.hid import CtapHidDevice
from fido2.ctap2 import Ctap2

ctap = Ctap2(next(CtapHidDevice.list_devices()))
print("algorithms:", [a["alg"] for a in ctap.get_info().algorithms])   # [-7, -8, -50]

rp   = {"id": "fido.demo", "name": "FIDO Demo"}
user = {"id": b"u1", "name": "u", "displayName": "u"}
cdh  = hashlib.sha256(b"demo").digest()

for alg in (-7, -8):          # ES256, EdDSA  -- touch for each
    att = ctap.make_credential(cdh, rp, user, [{"type": "public-key", "alg": alg}])
    print(f"alg {alg}: credential_id {len(att.auth_data.credential_data.credential_id)} bytes")
```
(`-50` is ML-DSA-44, **Hacker only** — see [POST_QUANTUM.md](POST_QUANTUM.md).)

## hmac-secret
`hmac-secret` lets the host derive a stable 32-byte secret from the credential + a salt, computed on the key (HMAC-SHA256). It's how FIDO2 powers symmetric encryption without storing keys on the host.

Request it at credential creation, then evaluate it at assertion:
```python
att = ctap.make_credential(cdh, rp, user, [{"type":"public-key","alg":-7}],
                           extensions={"hmac-secret": True})        # touch
# later, at get_assertion, supply a salt to get back the derived secret
# (the platform wraps the salt with a shared secret; see python-fido2's HmacSecret extension)
```
The easiest way to *use* hmac-secret is via [age + `age-plugin-fido2-hmac`](AGE.md), which handles the salt/derivation for you.

## CLI alternative
`libfido2`'s tools also work: `fido2-token -L` (list), `fido2-cred` (make credential), `fido2-assert` (assert).

See also: [SSH.md](SSH.md), [GIT.md](GIT.md), [AGE.md](AGE.md).
