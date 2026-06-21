# SSH with a Solo key

> **Works on: Secure + Hacker.** Standard FIDO2 (`-sk` keys); no special firmware.

Authenticate SSH with a hardware-backed key. The private key lives on the Solo and every login needs a **touch**. Needs OpenSSH ≥ 8.2 with FIDO support on **both** client and server.

## 1. Create the key
```bash
ssh-keygen -t ed25519-sk -O resident -C "$(whoami)@solo" -f ~/.ssh/id_ed25519_sk
```
Touch the Solo when it blinks. Use `-t ecdsa-sk` instead for P-256 if your OpenSSH lacks Ed25519-sk.
- `-O resident` — store on the key; recover later on another machine with `ssh-keygen -K`.
- `-O verify-required` — also require the Solo PIN on every use.

> **macOS:** Apple's `ssh`/`ssh-keygen` have no FIDO support (*"No FIDO SecurityKeyProvider specified"*). `brew install openssh` and put `/opt/homebrew/bin` first on `PATH`.

## 2. Install on a server
```bash
ssh-copy-id -i ~/.ssh/id_ed25519_sk.pub user@host     # or append the .pub to ~/.ssh/authorized_keys
```
For **GitHub** auth (clone/pull/push over SSH): add the `.pub` at GitHub → *Settings → SSH and GPG keys → New SSH key → Key type = Authentication Key*.

## 3. Log in
```bash
ssh user@host        # touch the Solo when prompted
```

## Recover the key on a new machine
With `-O resident`, pull the credential back from the Solo (no private key copied — it stays on the device):
```bash
cd ~/.ssh && ssh-keygen -K          # writes id_ed25519_sk[.pub], touch to confirm
```

See also [GIT.md](GIT.md) (commit signing) and [FIDO.md](FIDO.md) (the underlying FIDO2 algorithms).
