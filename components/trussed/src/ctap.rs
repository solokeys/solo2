List of stuff CTAP needs


// AES-256: 32 byte key
// ChaCha8Poly1305: 32 byte key

enum {
    Aes256Cbc,  // (secret, IV=0, data)
    HmacSha256, // (secret, saltEnc)

    // ChaCha8Poly1305, //
