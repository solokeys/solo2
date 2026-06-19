#!/usr/bin/env python3
"""Post-quantum signing on a SoloKey over PIV (ML-DSA-44), then verify with openssl + liboqs.

Flow:
  1. SELECT the PIV applet
  2. authenticate the management key (needed to generate a key)
  3. set a PIN  (change the default PIN -> your PIN, so signing is PIN-protected)
  4. generate an ML-DSA-44 key in slot 9A
  5. VERIFY PIN, then sign a message (GENERAL AUTHENTICATE)
  6. verify the signature with liboqs and with `openssl pkeyutl`

Requires: pyscard, liboqs-python (`oqs`), and OpenSSL >= 3.5 (native ML-DSA).
Usage:    python pq_piv_demo.py [NEW_PIN] [OLD_PIN]   (defaults: NEW=31415926, OLD=123456)
"""
import sys, subprocess, tempfile, os
from smartcard.System import readers

PIV_AID = bytes.fromhex("A000000308000010000100")
DEFAULT_MGMT_KEY = bytes.fromhex("010203040506070801020304050607080102030405060708")
ALG_MLDSA44 = 0xE0
SLOT_9A = 0x9A
OID_MLDSA44_DER = bytes.fromhex("0609608648016503040311")  # 2.16.840.1.101.3.4.3.17

NEW_PIN = (sys.argv[1] if len(sys.argv) > 1 else "31415926").encode()
OLD_PIN = (sys.argv[2] if len(sys.argv) > 2 else "123456").encode()
MESSAGE = b"post-quantum demo: signed on a SoloKey via PIV"


def ber_len(n):
    if n < 0x80: return bytes([n])
    if n < 0x100: return bytes([0x81, n])
    return bytes([0x82, (n >> 8) & 0xFF, n & 0xFF])

def parse_len(d, i):
    f = d[i]
    if f < 0x80: return f, i + 1
    n = f & 0x7F
    return int.from_bytes(d[i + 1:i + 1 + n], "big"), i + 1 + n

def send(conn, apdu, label=""):
    data, sw1, sw2 = conn.transmit(list(apdu))
    sw = (sw1 << 8) | sw2
    while (sw >> 8) == 0x61:  # GET RESPONSE chaining
        d, sw1, sw2 = conn.transmit([0x00, 0xC0, 0x00, 0x00, sw & 0xFF])
        data += d; sw = (sw1 << 8) | sw2
    if label: print(f"  {label}: SW={sw:04X} len={len(data)}")
    return bytes(data), sw

def pad8(pin): return pin + b"\xff" * (8 - len(pin))

def admin_auth(conn):
    from Crypto.Cipher import DES  # default mgmt key is 3 equal DES blocks -> single DES
    enc = lambda b: DES.new(DEFAULT_MGMT_KEY[:8], DES.MODE_ECB).encrypt(b)
    data, sw = send(conn, [0x00, 0x87, 0x03, 0x9B, 0x04, 0x7C, 0x02, 0x81, 0x00, 0x00], "mgmt challenge")
    assert sw == 0x9000
    i = data.index(0x81); clen = data[i + 1]; chal = bytes(data[i + 2:i + 2 + clen])
    resp = enc(chal); body = [0x7C, len(resp) + 2, 0x82, len(resp)] + list(resp)
    _, sw = send(conn, [0x00, 0x87, 0x03, 0x9B, len(body)] + body + [0x00], "mgmt auth")
    assert sw == 0x9000, "management-key auth failed"

def change_pin(conn, old, new):
    body = list(pad8(old) + pad8(new))
    _, sw = send(conn, [0x00, 0x24, 0x00, 0x80, len(body)] + body, "set PIN (change ref data)")
    if sw != 0x9000:
        raise SystemExit(f"  change PIN failed (SW={sw:04X}); is OLD_PIN correct / already changed?")

def verify_pin(conn, pin):
    _, sw = send(conn, [0x00, 0x20, 0x00, 0x80, 0x08] + list(pad8(pin)), "VERIFY PIN")
    assert sw == 0x9000, "PIN verify failed"

def generate(conn):
    body = [0xAC, 0x03, 0x80, 0x01, ALG_MLDSA44]
    data, sw = send(conn, [0x00, 0x47, 0x00, SLOT_9A, len(body)] + body + [0x00], "GENERATE ML-DSA-44")
    assert sw == 0x9000 and data[0] == 0x7F and data[1] == 0x49
    _, i = parse_len(data, 2); assert data[i] == 0x86
    klen, j = parse_len(data, i + 1)
    return bytes(data[j:j + klen])

def sign(conn, msg):
    inner = bytes([0x82, 0x00, 0x81]) + ber_len(len(msg)) + msg
    body = bytes([0x7C]) + ber_len(len(inner)) + inner
    data, sw = send(conn, [0x00, 0x87, ALG_MLDSA44, SLOT_9A] + list(ber_len(len(body))) + list(body) + [0x00], "SIGN")
    assert sw == 0x9000 and data[0] == 0x7C
    _, i = parse_len(data, 1); assert data[i] == 0x82
    slen, j = parse_len(data, i + 1)
    return bytes(data[j:j + slen])

def spki_der(raw_pub):  # wrap raw ML-DSA-44 pubkey into a SubjectPublicKeyInfo
    tlv = lambda t, v: bytes([t]) + ber_len(len(v)) + v
    return tlv(0x30, tlv(0x30, OID_MLDSA44_DER) + tlv(0x03, b"\x00" + raw_pub))

def main():
    conn = readers()[0].createConnection(); conn.connect()
    print("Connected:", readers()[0])
    send(conn, [0x00, 0xA4, 0x04, 0x00, len(PIV_AID)] + list(PIV_AID) + [0x00], "SELECT PIV")
    admin_auth(conn)
    change_pin(conn, OLD_PIN, NEW_PIN)            # <-- set up a real PIN first
    pub = generate(conn); assert len(pub) == 1312, len(pub)
    print(f"  ML-DSA-44 public key: {len(pub)} bytes")
    verify_pin(conn, NEW_PIN)                     # <-- signing is PIN-gated
    sig = sign(conn, MESSAGE); assert len(sig) == 2420, len(sig)
    print(f"  signature: {len(sig)} bytes")

    d = tempfile.mkdtemp(prefix="pq_piv_")
    msg_f, sig_f, pub_f = f"{d}/msg.bin", f"{d}/sig.bin", f"{d}/pub.der"
    open(msg_f, "wb").write(MESSAGE); open(sig_f, "wb").write(sig); open(pub_f, "wb").write(spki_der(pub))

    import oqs
    with oqs.Signature("ML-DSA-44") as v:
        print("  liboqs verify:", "PASS" if v.verify(MESSAGE, sig, pub) else "FAIL")
    r = subprocess.run(["openssl", "pkeyutl", "-verify", "-pubin", "-inkey", pub_f,
                        "-keyform", "DER", "-rawin", "-in", msg_f, "-sigfile", sig_f],
                       capture_output=True, text=True)
    print("  openssl verify:", r.stdout.strip() or r.stderr.strip())
    print(f"\nartifacts in {d}  (pub.der is an ML-DSA-44 SubjectPublicKeyInfo)")

if __name__ == "__main__":
    main()
