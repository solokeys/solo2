# File encryption with a Solo key + age

> **Works on: Secure + Hacker.** Uses FIDO2 `hmac-secret`.

[age](https://age-encryption.org) is a simple modern file-encryption tool. With [`age-plugin-fido2-hmac`](https://github.com/olastor/age-plugin-fido2-hmac) the decryption key is derived **on the Solo** (via `hmac-secret`), so files can only be decrypted with a touch of the key.

## Install
```bash
brew install age                              # or your package manager
# install age-plugin-fido2-hmac from its releases / cargo; must be on PATH as `age-plugin-fido2-hmac`
```

## Create an identity bound to the Solo
```bash
age-plugin-fido2-hmac -g                      # touch the Solo
```
This prints a **recipient** (`age1fido2-hmac1…`, share/encrypt to it) and an **identity** (`AGE-PLUGIN-FIDO2-HMAC-…`, keep it; it's a handle, the secret stays on the key). Save the identity to `key.txt`.

## Encrypt / decrypt
```bash
age -r age1fido2-hmac1…  -o secret.age  secret.txt      # encrypt to the recipient
age -d -i key.txt        -o secret.txt  secret.age      # decrypt — touch the Solo
```

> Exact flags vary slightly by plugin version (some versions use `-j fido2-hmac` or a symmetric mode); check `age-plugin-fido2-hmac --help`. The plugin uses the credential's `hmac-secret` ([FIDO.md](FIDO.md)) to derive the file key.

## Where this is used
- **Encrypted backups:** wrap a `borg`/`restic` repo key with age.
- **Repo secrets:** `sops` with an age recipient.
- **Passwords:** [`passage`](PASS.md) is `pass` built on age — pair it with this for a touch-to-unlock password store.
