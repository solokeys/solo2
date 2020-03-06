import fido2.attestation
import fido2.ctap2
import fido2.hid

dev = fido2.ctap2.CTAP2(next(fido2.hid.CtapHidDevice.list_devices()))

print(dev.get_info())
# print(dev.reset())

for alg in (-7, -8):
    att = dev.make_credential(
        b"1234567890ABCDEF1234567890ABCDEF",
        {"id": "https://yamnord.com"},
        {"id": b"nickray"},
        [{"type": "public-key", "alg": alg}],
    )

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
        allow_list=[
            {"type": "public-key", "id": att.auth_data.credential_data.credential_id}
        ],
    )

    # basic sanity check - would raise
    assn.verify(client_data_hash, att.auth_data.credential_data.public_key)
