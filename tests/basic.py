import cbor
import fido2.attestation
import fido2.ctap2
import fido2.hid
import IPython

dev = fido2.ctap2.CTAP2(next(fido2.hid.CtapHidDevice.list_devices()))

print(dev.get_info())

# IPython.embed()

# quit()
# xxx

P256 = -7
Ed25519 = -8

# print(dev.reset())

# for alg in (-7, -8):
# for alg in (P256, Ed25519):

credential_ids = []
public_keys = []

for alg in (Ed25519, P256):
    att = dev.make_credential(
        b"1234567890ABCDEF1234567890ABCDEF",
        {"id": "https://yamnord.com"},
        {"id": b"nickray"},
        [{"type": "public-key", "alg": alg}],
        extensions={"hmac-secret": True},
        options={"rk": True},
    )

    credential_id = att.auth_data.credential_data.credential_id
    credential_ids.append(credential_id)

    public_key = att.auth_data.credential_data.public_key
    public_keys.append(public_key)

    # basic sanity check - would raise
    assert att.fmt == "packed"
    verifier = fido2.attestation.Attestation.for_type(att.fmt)()
    verifier.verify(
        att.att_statement, att.auth_data, b"1234567890ABCDEF1234567890ABCDEF"
    )

    client_data_hash = b"some_client_data_hash_abcdefghij"
    assn = dev.get_assertion(
        "https://yamnord.com",
        client_data_hash,
        allow_list=[{"type": "public-key", "id": credential_id}],
    )

    # basic sanity check - would raise
    assn.verify(client_data_hash, public_key)

# # GA/GNA combo
# assn = dev.get_assertion("https://yamnord.com", client_data_hash)
# assn.verify(client_data_hash, public_keys[1])

# assn = dev.get_next_assertion()
# assn.verify(client_data_hash, public_keys[0])

print(":: RESET ::")
dev.reset()
