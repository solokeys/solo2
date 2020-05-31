import base64
import datetime
import sys

import cbor
import fido2.attestation
import fido2.ctap2
import fido2.hid
import fido2.webauthn
import IPython
import pyotp

pin = "1234"

# P256 = -7
# Ed25519 = -8
# officially, "less than -65536" is reserved for private use
alg = TOTP = -9  # this is "unassigned"

# in google authenticator, these 32B are a Base32 encoding
# of 20 raw bytes
SECRETb32 = "FVLQ54PA26B74CORYFK36CCUL4DXNMWG"
SECRET = base64.b32decode(SECRETb32)
CLIENT_DATA_HASH = b"TOTP--" + SECRET + b"--TOTP"

dev = fido2.ctap2.CTAP2(next(fido2.hid.CtapHidDevice.list_devices()))


PP = fido2.ctap2.PinProtocolV1
pp = PP(dev)

dev_info = dev.get_info()

# dev.reset()

credential_ids = []
public_keys = []

# IPython.embed()
print(f"MC for {alg}")
att = dev.make_credential(
    # client data hash, officially contains:
    # - type, e.g. "webauthn.create"
    # - challenge, base64url encoding of at least 16B entropy
    # - origin (scheme, host, port, domain)
    # - token binding
    # hash is sha256, e.g. 32B or 64 hex characters
    # b"1234567890ABCDEF1234567890ABCDEF",
    CLIENT_DATA_HASH,

    # RP
    {"id": "yamnord.com", "name": "Yamnord"},

    # user
    {
        "id": b"2347asdf7234",
        "name": "nickray",
        "displayName": "nickray",
    },
    # key params
    [{"type": "public-key", "alg": alg}],
    # can't rely on unknown extensions being passed through
    # extensions={"hmac-secret": True},
    options={"rk": True},
)

credential_id = att.auth_data.credential_data.credential_id

if False:
    assert att.fmt == "packed"
    verifier = fido2.attestation.Attestation.for_type(att.fmt)()
    # This doesn't work, "InvalidData: Wrong algorithm of public key!"
    # as pub_key.ALGORITHM != alg
    # (we currently only do self-signed attestations)
    verifier.verify(
        att.att_statement, att.auth_data, CLIENT_DATA_HASH
    )

# this fits in a u64
timestamp = int(datetime.datetime.utcnow().timestamp())
padding = b"\0"*24
client_data_hash = timestamp.to_bytes(8, byteorder="little") + padding
print(f"trying TOTP with timestamp {timestamp}")
assn = dev.get_assertion(
    "yamnord.com",
    client_data_hash,
    allow_list=[{"type": "public-key", "id": credential_id}],
)

otp = str(int.from_bytes(assn.signature[:8], "little")).zfill(6)

hotp = pyotp.HOTP(SECRETb32)
correct = hotp.verify(otp, timestamp)
print(f"Solo calculated the correct TOTP? {correct}")
# IPython.embed()

sys.exit(not correct)

if False:
    # basic sanity check - would raise
    assn.verify(client_data_hash, public_key)


