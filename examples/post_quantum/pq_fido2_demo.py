#!/usr/bin/env python3
"""Post-quantum signing on a SoloKey over FIDO2 (ML-DSA-44 = COSE alg -48),
then verify the assertion with liboqs + openssl.

Flow:
  1. get_info  -> confirm the authenticator advertises alg -48 (ML-DSA-44)
  2. MakeCredential with pubKeyCredParams = [{alg: -48}]   <-- TOUCH the Solo
  3. GetAssertion for that credential                       <-- TOUCH the Solo
  4. verify the assertion signature over (authData || SHA256(clientDataJSON))
     with liboqs and with `openssl pkeyutl`

FIDO2 mandates user-presence, so MakeCredential and GetAssertion each require a
physical touch. Requires: python-fido2, liboqs-python (`oqs`), OpenSSL >= 3.5.
"""
import hashlib, json, os, subprocess, tempfile
from fido2.hid import CtapHidDevice
from fido2.ctap2 import Ctap2

RP = {"id": "pq.demo", "name": "PQ Demo"}
USER = {"id": b"pquser01", "name": "pq", "displayName": "PQ User"}
OID_MLDSA44_DER = bytes.fromhex("0609608648016503040311")  # 2.16.840.1.101.3.4.3.17

def ber_len(n):
    if n < 0x80: return bytes([n])
    if n < 0x100: return bytes([0x81, n])
    return bytes([0x82, (n >> 8) & 0xFF, n & 0xFF])

def spki_der(raw_pub):
    tlv = lambda t, v: bytes([t]) + ber_len(len(v)) + v
    return tlv(0x30, tlv(0x30, OID_MLDSA44_DER) + tlv(0x03, b"\x00" + raw_pub))

def raw_mldsa_pubkey(cose_key):
    # the ML-DSA-44 public key is the 1312-byte field in the COSE_Key map
    for v in (cose_key.values() if hasattr(cose_key, "values") else []):
        if isinstance(v, (bytes, bytearray)) and len(v) == 1312:
            return bytes(v)
    raise SystemExit(f"could not find 1312-byte ML-DSA pubkey in COSE key: {cose_key}")

def pick_ctap():
    """Pick the key that advertises ML-DSA-44. Enumeration order is not stable,
    so select by capability; SOLO_AAGUID=<hex prefix> forces a specific key."""
    want = os.environ.get("SOLO_AAGUID", "").lower()
    others = []
    for dev in CtapHidDevice.list_devices():
        try:
            ctap = Ctap2(dev)
            info = ctap.get_info()
        except Exception:
            continue
        aaguid = info.aaguid.hex()
        algs = [a.get("alg") for a in (info.algorithms or [])]
        if want:
            if aaguid.startswith(want):
                return ctap, algs
        elif -48 in algs:
            return ctap, algs
        others.append(f"{aaguid[:8]} {algs}")
    raise SystemExit(
        "no key advertising ML-DSA-44 (-48); found: " + (", ".join(others) or "none")
        + "\nthis demo needs Hacker firmware (mldsa44-fido)"
    )

def main():
    ctap, algs = pick_ctap()
    print("authenticator algorithms:", algs)

    cdh1 = hashlib.sha256(b"pq-fido2-demo-create").digest()
    print(">> MakeCredential (alg -48) — TOUCH the Solo ...")
    att = ctap.make_credential(cdh1, RP, USER, [{"type": "public-key", "alg": -48}])
    cd = att.auth_data.credential_data
    pub = raw_mldsa_pubkey(cd.public_key)
    print(f"   credential_id: {len(cd.credential_id)} bytes; ML-DSA pubkey: {len(pub)} bytes")

    cdh2 = hashlib.sha256(b"pq-fido2-demo-assert").digest()
    print(">> GetAssertion — TOUCH the Solo ...")
    asr = ctap.get_assertion(RP["id"], cdh2,
                             allow_list=[{"type": "public-key", "id": cd.credential_id}])
    signed = bytes(asr.auth_data) + cdh2          # WebAuthn assertion signature base
    sig = asr.signature
    print(f"   signature: {len(sig)} bytes over (authData || clientDataHash)")

    import oqs
    with oqs.Signature("ML-DSA-44") as v:
        print("   liboqs verify:", "PASS" if v.verify(signed, sig, pub) else "FAIL")

    d = tempfile.mkdtemp(prefix="pq_fido2_")
    open(f"{d}/msg.bin", "wb").write(signed); open(f"{d}/sig.bin", "wb").write(sig)
    open(f"{d}/pub.der", "wb").write(spki_der(pub))
    r = subprocess.run(["openssl", "pkeyutl", "-verify", "-pubin", "-inkey", f"{d}/pub.der",
                        "-keyform", "DER", "-rawin", "-in", f"{d}/msg.bin", "-sigfile", f"{d}/sig.bin"],
                       capture_output=True, text=True)
    print("   openssl verify:", r.stdout.strip() or r.stderr.strip())

if __name__ == "__main__":
    main()
