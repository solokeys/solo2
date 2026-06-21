# Post-quantum transparency-log witness with a Solo key

> **Works on: Secure + Hacker.** Uses the PIV ML-DSA path, which is part of the Secure baseline (needs the ML-DSA firmware — see [POST_QUANTUM.md](POST_QUANTUM.md)).

A transparency-log **witness** ([c2sp.org/tlog-witness](https://c2sp.org/tlog-witness)) cosigns a log's checkpoint so clients can detect a log that shows different trees to different people (a split view). The cosignature format ([c2sp.org/tlog-cosignature](https://c2sp.org/tlog-cosignature)) defines **ML-DSA-44 as signature type `0x06`** — the post-quantum algorithm a Solo key can sign with. So a Solo can be a **post-quantum witness key**.

## How a cosignature is built
For a checkpoint note whose body is

```
go.sum database tree
55905176
ZVT1A3aWwsywOo9HPzBGAi9ymb8nHfUHh1My/Hh7/yk=
```

the witness signs the message `cosignature/v1\ntime <unixsecs>\n` + that body, and appends a line

```
— <name> base64( keyID[4] || timestamp_u64_BE[8] || signature )
```

where `keyID = SHA-256(name || "\n" || 0x06 || pubkey)[:4]`. ML-DSA-44 keys are 1312 bytes, signatures 2420 bytes.

## Run it
The demo fetches a **real** checkpoint from the Go checksum database, cosigns it on the Solo over PIV (PIN-gated, no touch), and verifies three independent ways.

```bash
# liboqs + pyscard in a venv; OpenSSL >= 3.5 on PATH
python post_quantum/pq_tlog_witness.py             # cosign the real sum.golang.org checkpoint on the Solo
python post_quantum/pq_tlog_witness.py --software  # no device: software ML-DSA key, to see the construction
```

It prints the cosigned note and:

```
Verification
  liboqs : PASS
  openssl: Signature Verified Successfully
```

## Independent verification (reference ecosystem)
There is no off-the-shelf ML-DSA cosignature verifier yet ([filippo.io/torchwood](https://pkg.go.dev/filippo.io/torchwood) is Ed25519-only so far), so `post_quantum/tlog_verify/` is a ~90-line Go checker built on **[filippo.io/mldsa](https://pkg.go.dev/filippo.io/mldsa)** (the proposed `crypto/mldsa`). It re-parses the note, recomputes the key ID, reconstructs the signed message, and verifies — a separate codebase from the Python that produced it, so agreement is a real cross-implementation check.

```bash
cd post_quantum/tlog_verify
go run . -note <cosigned.txt> -name witness.example/solo-mldsa -pub <witness.pub>
#   OK  ML-DSA-44 cosignature verified (filippo.io/mldsa)
```

## Notes
- A production witness verifies a Merkle **consistency proof** against the last size it cosigned, and persists state, before signing (see [c2sp.org/tlog-witness](https://c2sp.org/tlog-witness)). This demo cosigns the real checkpoint body to show the post-quantum cosignature path itself.
- The demo generates a fresh witness key in PIV slot 9A each run; a real witness keeps a stable key.

See also: [POST_QUANTUM.md](POST_QUANTUM.md), [FIDO.md](FIDO.md).
