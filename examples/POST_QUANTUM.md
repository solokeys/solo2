# Post-Quantum signing with a SoloKey (ML-DSA-44)

> **Works on:** PIV ML-DSA — **Secure + Hacker**; FIDO2 ML-DSA (`-50`) — **Hacker only**. ML-DSA needs the ML-DSA firmware; the FIDO2 `-50` algorithm is pre-standard CTAP, so it's excluded from the certified Secure build, while PIV ML-DSA is part of the Secure baseline.

Solo 2 supports **ML-DSA-44** (FIPS-204) signatures via both **FIDO2** (COSE alg `-50`) and **PIV**, powered by [libcrux](https://github.com/cryspen/libcrux).

The key signs; **standard tooling verifies** — the signatures are plain FIPS-204 ML-DSA (empty context), so OpenSSL ≥ 3.5 and liboqs verify them directly.

## Prerequisites
- Firmware with ML-DSA: PIV (`mldsa44-piv`) is in the **Secure** baseline; FIDO2 `-50` (`mldsa44-fido`) is **Hacker** only.
- Host tools: `python-fido2`, `pyscard`, [`liboqs-python`](https://github.com/open-quantum-safe/liboqs-python) (`oqs`), and **OpenSSL ≥ 3.5** (native ML-DSA — `openssl list -signature-algorithms | grep ML-DSA`).
- Scripts: [`pq_fido2_demo.py`](post_quantum/pq_fido2_demo.py), [`pq_piv_demo.py`](post_quantum/pq_piv_demo.py).

---

## FIDO2 demo (alg -50) — Hacker only

FIDO2 mandates user-presence, so **MakeCredential** and **GetAssertion** each require a **physical touch**. The script: confirms the authenticator advertises `-50`, makes an ML-DSA credential, gets an assertion, and verifies the assertion signature — computed over `authData || SHA256(clientDataJSON)` — with liboqs and openssl.

```bash
python post_quantum/pq_fido2_demo.py
```
```
authenticator algorithms: [-7, -8, -50]
>> MakeCredential (alg -50) — TOUCH the Solo ...
   credential_id: 159 bytes; ML-DSA pubkey: 1312 bytes
>> GetAssertion — TOUCH the Solo ...
   signature: 2420 bytes over (authData || clientDataHash)
   liboqs verify: PASS
   openssl verify: Signature Verified Successfully
```

The 1312-byte public key and 2420-byte signature are the ML-DSA-44 sizes. To do it by hand, a `MakeCredential` with `pubKeyCredParams = [{"type":"public-key","alg":-50}]` returns the ML-DSA COSE key in `authData`; a `GetAssertion` returns the raw ML-DSA signature.

---

## PIV demo — Secure + Hacker

PIV signing is **PIN-protected**: we set a PIN first, then every signature requires `VERIFY PIN`. The script: authenticates the management key, **sets a PIN** (changes the default), generates an ML-DSA-44 key in slot `9A`, verifies the PIN, signs, and verifies with liboqs + openssl.

```bash
python post_quantum/pq_piv_demo.py 31415926   # arg = the PIN to set
```
```
Connected: SoloKeys Solo 2 Security Key
  SELECT PIV: SW=9000 len=107
  mgmt challenge: SW=9000 len=12
  mgmt auth: SW=9000 len=0
  set PIN (change ref data): SW=9000 len=0
  GENERATE ML-DSA-44: SW=9000 len=1321
  ML-DSA-44 public key: 1312 bytes
  VERIFY PIN: SW=9000 len=0
  SIGN: SW=9000 len=2428
  signature: 2420 bytes
  liboqs verify: PASS
  openssl verify: Signature Verified Successfully
```

### Verify with `openssl` directly
The card exports a raw 1312-byte public key. Wrap it in a `SubjectPublicKeyInfo` (`AlgorithmIdentifier{id-ml-dsa-44}` + `BIT STRING`) and OpenSSL loads it as a native key:

```bash
openssl pkey -pubin -inform DER -in pub.der -text -noout     # -> "ML-DSA-44 Public-Key:"
openssl pkeyutl -verify -pubin -inkey pub.der -keyform DER \
        -rawin -in msg.bin -sigfile sig.bin                  # -> Signature Verified Successfully
```

`-rawin` makes OpenSSL run pure ML-DSA.Verify over the message with the (default) empty context — byte-for-byte what the SoloKey computes.

### Verify with liboqs (Python)
```python
import oqs
with oqs.Signature("ML-DSA-44") as v:
    assert v.verify(message, signature, raw_pubkey)   # True
```

---

Both paths use the same on-device ML-DSA-44 implementation; the difference is only the protocol surface (FIDO2 vs PIV) and how the signed message is framed.

For a real-world use of the PIV ML-DSA path, see [TLOG.md](TLOG.md): the Solo as a post-quantum transparency-log witness.
