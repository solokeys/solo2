#!/usr/bin/env python3
"""Use a SoloKey as a post-quantum transparency-log witness.

A transparency-log *witness* (c2sp.org/tlog-witness) cosigns a log's checkpoint to
prevent split-view attacks. The cosignature format (c2sp.org/tlog-cosignature) now
defines ML-DSA-44 as signature type 0x06 -- the exact algorithm a SoloKey can sign
with. So a Solo can act as a post-quantum witness key.

This demo:
  1. fetches a REAL checkpoint from the Go checksum database (sum.golang.org/latest)
  2. builds the cosignature signed message: "cosignature/v1\\ntime <ts>\\n" + body
  3. signs it with ML-DSA-44 on the Solo over PIV  (PIN-gated; no touch needed)
  4. assembles the cosignature line "— <name> base64(keyID || ts || sig)"
  5. verifies it with OpenSSL (>=3.5) and liboqs, and emits the note for the
     independent Go verifier in ./tlog_verify (filippo.io/mldsa).

NOTE: a production witness verifies a consistency proof before cosigning; this demo
cosigns the real checkpoint body to show the post-quantum cosignature path itself.

Requires: pyscard, liboqs-python (`oqs`), OpenSSL >= 3.5, and ML-DSA firmware
(PIV ML-DSA is in the Secure baseline; also on Hacker).
Usage:
  python pq_tlog_witness.py                 # sign on the Solo
  python pq_tlog_witness.py --software      # no device: software ML-DSA key (test the construction)
  python pq_tlog_witness.py --time 1700000000   # fixed timestamp (reproducible)
"""
import argparse, hashlib, struct, subprocess, tempfile, time, urllib.request, base64, os, sys

NAME = "witness.example/solo-mldsa"   # a schema-less URL identifying the cosigner
MLDSA44_TYPE = 0x06
CHECKPOINT_URL = "https://sum.golang.org/latest"

# --- PIV ML-DSA signing path, shared with pq_piv_demo.py -------------------------
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def fetch_checkpoint():
    with urllib.request.urlopen(CHECKPOINT_URL, timeout=15) as r:
        return r.read()


def checkpoint_body(note: bytes) -> bytes:
    # body = text up to (and including the newline before) the blank line that
    # separates a signed note's body from its signature lines.
    sep = note.index(b"\n\n")
    return note[:sep + 1]


def key_id(name: str, pub: bytes) -> bytes:
    return hashlib.sha256(name.encode() + b"\n" + bytes([MLDSA44_TYPE]) + pub).digest()[:4]


def cosig_line(name: str, pub: bytes, ts: int, sig: bytes) -> str:
    value = key_id(name, pub) + struct.pack(">Q", ts) + sig   # keyID || u64 BE || sig
    return f"— {name} {base64.standard_b64encode(value).decode()}"


def sign_solo(message: bytes):
    from smartcard.System import readers
    from pq_piv_demo import send, admin_auth, change_pin, verify_pin, generate, sign, PIV_AID
    rs = readers()
    if not rs:
        raise SystemExit("no PC/SC reader -- plug in the Solo (it presents as a CCID reader)")
    conn = rs[0].createConnection(); conn.connect()
    print("Connected:", rs[0])
    send(conn, [0x00, 0xA4, 0x04, 0x00, len(PIV_AID)] + list(PIV_AID) + [0x00], "SELECT PIV")
    admin_auth(conn)
    try:
        change_pin(conn, b"123456", b"31415926")   # set a real PIN on first run
    except SystemExit:
        pass                                        # already changed -> reuse it
    pub = generate(conn); assert len(pub) == 1312, len(pub)
    print(f"  ML-DSA-44 witness key: {len(pub)} bytes (PIV slot 9A)")
    verify_pin(conn, b"31415926")
    sig = sign(conn, message); assert len(sig) == 2420, len(sig)
    print(f"  cosignature: {len(sig)} bytes")
    return pub, sig


def sign_software(message: bytes):
    import oqs
    s = oqs.Signature("ML-DSA-44")
    pub = s.generate_keypair()
    print(f"  ML-DSA-44 software key: {len(pub)} bytes (no device)")
    sig = s.sign(message)
    return pub, sig


def spki_der(pub: bytes) -> bytes:
    oid = bytes.fromhex("0609608648016503040311")            # 2.16.840.1.101.3.4.3.17
    tlv = lambda t, v: bytes([t]) + (bytes([len(v)]) if len(v) < 0x80 else
        bytes([0x80 | (len(v).bit_length() + 7) // 8]) + len(v).to_bytes((len(v).bit_length() + 7) // 8, "big")) + v
    return tlv(0x30, tlv(0x30, oid) + tlv(0x03, b"\x00" + pub))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--software", action="store_true", help="use a software ML-DSA key (no device)")
    ap.add_argument("--time", type=int, default=None, help="cosignature timestamp (default: now)")
    args = ap.parse_args()

    note = fetch_checkpoint()
    body = checkpoint_body(note)
    print("Real checkpoint from", CHECKPOINT_URL)
    print("  origin/size/root:", body.decode().strip().replace("\n", " | "))

    ts = args.time if args.time is not None else int(time.time())
    message = b"cosignature/v1\ntime %d\n" % ts + body

    pub, sig = sign_software(message) if args.software else sign_solo(message)

    cosigned = note + (cosig_line(NAME, pub, ts, sig) + "\n").encode()

    d = tempfile.mkdtemp(prefix="pq_tlog_")
    note_f, pub_f, der_f, msg_f, sig_f = (f"{d}/cosigned.txt", f"{d}/witness.pub",
                                          f"{d}/witness.der", f"{d}/msg.bin", f"{d}/sig.bin")
    open(note_f, "wb").write(cosigned)
    open(pub_f, "wb").write(pub)
    open(der_f, "wb").write(spki_der(pub))
    open(msg_f, "wb").write(message)
    open(sig_f, "wb").write(sig)

    print("\nCosigned checkpoint:")
    print("  " + cosigned.decode().replace("\n", "\n  ").rstrip())

    print("\nVerification")
    import oqs
    with oqs.Signature("ML-DSA-44") as v:
        print("  liboqs :", "PASS" if v.verify(message, sig, pub) else "FAIL")
    r = subprocess.run(["openssl", "pkeyutl", "-verify", "-pubin", "-inkey", der_f,
                        "-keyform", "DER", "-rawin", "-in", msg_f, "-sigfile", sig_f],
                       capture_output=True, text=True)
    print("  openssl:", r.stdout.strip() or r.stderr.strip())

    print(f"\nIndependent Go verifier (reference ecosystem, filippo.io/mldsa):")
    print(f"  ( cd {os.path.dirname(os.path.abspath(__file__))}/tlog_verify && \\")
    print(f"    go run . -note {note_f} -name {NAME} -pub {pub_f} )")
    print(f"\nartifacts in {d}")


if __name__ == "__main__":
    main()
