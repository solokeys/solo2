#!/usr/bin/env python3
"""Serves webauthn_mldsa.html and verifies ML-DSA-44 WebAuthn signatures with liboqs.

    ~/code/solo/solo-ws3/.mldsa-venv/bin/python webauthn_verify_server.py
    open http://localhost:8443/webauthn_mldsa.html

The browser cannot check an ML-DSA signature (no SPKI parser, hence
getPublicKey() == null), so the page POSTs the credential and assertion here.
"""
import base64
import hashlib
import http.server
import json

import oqs
from fido2.cbor import decode as cbor_decode
from fido2.cbor import decode_from as cbor_decode_from

CREDS = {}  # credential id -> (raw public key, cose alg)


def b64u(s):
    return base64.urlsafe_b64decode(s + "=" * (-len(s) % 4))


def parse_attestation(att_obj):
    """attestationObject -> (credential id, COSE key dict)."""
    att = cbor_decode(att_obj)
    auth = att["authData"]
    id_len = int.from_bytes(auth[53:55], "big")
    cred_id = auth[55 : 55 + id_len]
    cose, _rest = cbor_decode_from(auth[55 + id_len :])  # extensions may follow
    return cred_id, cose


class Handler(http.server.SimpleHTTPRequestHandler):
    def _json(self, obj, code=200):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        req = json.loads(self.rfile.read(n) or b"{}")
        try:
            if self.path == "/register":
                cred_id, cose = parse_attestation(b64u(req["attestationObject"]))
                alg = cose.get(3)
                # COSE_Key for ML-DSA (draft-ietf-cose-akp): kty 7, key bytes in -1
                pub = cose.get(-1) if alg == -48 else None
                CREDS[cred_id.hex()] = (pub, alg)
                self._json({
                    "alg": alg,
                    "kty": cose.get(1),
                    "pubkey_len": len(pub) if pub else None,
                    "cred_id": cred_id.hex()[:32],
                    "note": "ML-DSA-44 public key is 1312 bytes" if alg == -48 else "not ML-DSA",
                })
            elif self.path == "/verify":
                cred_id = bytes.fromhex(req["credId"])
                pub, alg = CREDS.get(cred_id.hex(), (None, None))
                if pub is None:
                    return self._json({"error": "unknown credential or not ML-DSA"}, 400)
                auth_data = b64u(req["authenticatorData"])
                client_data = b64u(req["clientDataJSON"])
                sig = b64u(req["signature"])
                # WebAuthn: signature is over authenticatorData || SHA256(clientDataJSON)
                signed = auth_data + hashlib.sha256(client_data).digest()
                with oqs.Signature("ML-DSA-44") as v:
                    ok = v.verify(signed, sig, pub)
                self._json({
                    "verified": bool(ok),
                    "alg": alg,
                    "sig_len": len(sig),
                    "signed_len": len(signed),
                })
            else:
                self._json({"error": "unknown endpoint"}, 404)
        except Exception as e:  # surface the real reason to the page
            self._json({"error": f"{type(e).__name__}: {e}"}, 500)

    def log_message(self, fmt, *args):
        print("  " + fmt % args)


if __name__ == "__main__":
    print("serving http://localhost:8443/webauthn_mldsa.html")
    http.server.HTTPServer(("127.0.0.1", 8443), Handler).serve_forever()
