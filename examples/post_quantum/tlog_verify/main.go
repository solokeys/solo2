// Command tlog_verify independently verifies an ML-DSA-44 tlog-cosignature
// (c2sp.org/tlog-cosignature, signature type 0x06) on a signed-note checkpoint.
//
// It is deliberately separate from the Python that *produces* the cosignature:
// agreement between the two is the cross-implementation interop check. The
// ML-DSA-44 math is filippo.io/mldsa (the proposed crypto/mldsa).
//
//	go run . -note cosigned.txt -name witness.example/solo-mldsa -pub witness.pub
package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"encoding/binary"
	"flag"
	"fmt"
	"os"

	"filippo.io/mldsa"
)

const mldsa44Type = 0x06

func main() {
	noteFile := flag.String("note", "", "cosigned checkpoint note file")
	name := flag.String("name", "", "cosigner key name")
	pubFile := flag.String("pub", "", "cosigner public key file (1312 raw bytes)")
	flag.Parse()
	if *noteFile == "" || *name == "" || *pubFile == "" {
		flag.Usage()
		os.Exit(2)
	}

	note, err := os.ReadFile(*noteFile)
	check(err)
	pub, err := os.ReadFile(*pubFile)
	check(err)
	if len(pub) != 1312 {
		fatal("public key is %d bytes, want 1312 (ML-DSA-44)", len(pub))
	}

	// A signed note is: body text, a blank line, then "— name base64" lines.
	// The cosignature signs the body (up to and including the line before the
	// blank line), never the signatures.
	sep := bytes.Index(note, []byte("\n\n"))
	if sep < 0 {
		fatal("no blank line separating body from signatures")
	}
	body := note[:sep+1]

	// Find the cosignature line for this name.
	prefix := []byte("— " + *name + " ")
	var b64 []byte
	for _, line := range bytes.Split(note[sep+2:], []byte("\n")) {
		if bytes.HasPrefix(line, prefix) {
			b64 = line[len(prefix):]
			break
		}
	}
	if b64 == nil {
		fatal("no cosignature line for %q", *name)
	}
	val, err := base64.StdEncoding.DecodeString(string(b64))
	check(err)
	if len(val) != 4+8+2420 {
		fatal("cosignature value is %d bytes, want %d (keyID+timestamp+sig)", len(val), 4+8+2420)
	}
	keyID, ts, sig := val[:4], binary.BigEndian.Uint64(val[4:12]), val[12:]

	// Expected key ID: SHA-256(name || "\n" || 0x06 || pubkey)[:4].
	h := sha256.New()
	h.Write([]byte(*name))
	h.Write([]byte{'\n', mldsa44Type})
	h.Write(pub)
	wantID := h.Sum(nil)[:4]
	if !bytes.Equal(keyID, wantID) {
		fatal("key ID %x does not match expected %x for this name+pubkey", keyID, wantID)
	}

	// Signed message: "cosignature/v1\ntime <ts>\n" + body.
	msg := append([]byte(fmt.Sprintf("cosignature/v1\ntime %d\n", ts)), body...)

	pk, err := mldsa.NewPublicKey(mldsa.MLDSA44(), pub)
	check(err)
	if err := mldsa.Verify(pk, msg, sig, nil); err != nil {
		fatal("ML-DSA-44 verify FAILED: %v", err)
	}
	fmt.Printf("OK  ML-DSA-44 cosignature verified (filippo.io/mldsa)\n")
	fmt.Printf("    cosigner:  %s\n", *name)
	fmt.Printf("    key ID:    %x\n", keyID)
	fmt.Printf("    timestamp: %d\n", ts)
}

func check(err error) {
	if err != nil {
		fatal("%v", err)
	}
}

func fatal(f string, a ...any) {
	fmt.Fprintf(os.Stderr, "FAIL: "+f+"\n", a...)
	os.Exit(1)
}
