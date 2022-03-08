# import cbor
import fido2.attestation
import fido2.ctap2
import fido2.hid
import fido2.webauthn
import IPython

pin = "1234"

dev = fido2.ctap2.CTAP2(next(fido2.hid.CtapHidDevice.list_devices()))

client_pin = fido2.ctap2.ClientPin(dev)

dev_info = dev.get_info()

N = 3

print(dev_info)
if dev_info.options.get('clientPin', False):
    pin_token = client_pin.get_pin_token(pin)
    print(f"PIN set, token = {pin_token}")
    print("resetting device to clear PIN")
    dev.reset()

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

# for alg in (Ed25519, P256):
for alg in (P256, Ed25519):
# for alg in (Ed25519,):
    print(f"MC for {alg}")
    att = dev.make_credential(
        b"1234567890ABCDEF1234567890ABCDEF",
        {"id": "yamnord.com", "name": "Yamnord"},
        {
            "id": b"nickray",
            "icon": "https://yamnord.com/favicon/favicon-32x32.png",
            "name": "nickray",
            "displayName": "nickray",
        },
        [{"type": "public-key", "alg": alg}],
        extensions={"hmac-secret": True},
        options={"rk": True},
    )

    credential_id = att.auth_data.credential_data.credential_id
    print(att.auth_data.credential_data)
    print(f":: Credential ID (len{len(credential_id)}):\n{credential_id.hex()}")
    credential_ids.append(credential_id)

    public_key = att.auth_data.credential_data.public_key
    public_keys.append(public_key)

    # basic sanity check - would raise
    assert att.fmt == "packed"
    verifier = fido2.attestation.Attestation.for_type(att.fmt)()
    # verifier.verify(
    #     att.att_statement, att.auth_data, b"1234567890ABCDEF1234567890ABCDEF"
    # )

    client_data_hash = b"some_client_data_hash_abcdefghij"
    for _ in range(N):
        assn = dev.get_assertion(
            "yamnord.com",
            client_data_hash,
            allow_list=[{"type": "public-key", "id": credential_id}],
        )

        # basic sanity check - would raise
        assn.verify(client_data_hash, public_key)

# GA/GNA combo
assn = dev.get_assertion("yamnord.com", client_data_hash)
# assn.verify(client_data_hash, public_keys[1])

# assn = dev.get_next_assertion()
# assn.verify(client_data_hash, public_keys[0])


# make another RP
dev.make_credential(
    b"1234567890ABCDEF1234567890ABCDEF",
    {"id": "yamnord.com", "name": "Yamnord"},
    {"id": b"nickray", "name": "nickray", "displayName": "nickray"},
    [{"type": "public-key", "alg": alg}],
    extensions={"hmac-secret": True},
    options={"rk": True},
)

# print(":: RESET ::")
# dev.reset()

# PP = fido2.ctap2.PinProtocolV1
# pp = PP(dev)
try:
    client_pin.set_pin(pin)
except Exception as e:
    print("pin already set")
    pass

try:
    client_pin.set_pin(pin)
except Exception as e:
    print("pin already set")
    pass

# print(pp.get_shared_secret())
# pin_token = pp.get_pin_token(pin)
# print(pin_token)


# we reset, so need new pin token!!
pin_token = client_pin.get_pin_token(pin)

cm = fido2.ctap2.CredentialManagement(dev, client_pin, pin_token)
# rp0 = dev.credential_mgmt(CM.CMD.ENUMERATE_RPS_BEGIN)
# print(rp0)

# import fido2.webauthn
cd = fido2.webauthn.PublicKeyCredentialDescriptor("public-key", credential_ids[0])
# cd1 = fido2.webauthn.PublicKeyCredentialDescriptor("public-key", credential_ids[1])
# cm.delete_cred(cd)
