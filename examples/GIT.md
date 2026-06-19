# Sign Git commits with a SoloKey

> **Works on: Secure + Hacker.** Uses standard FIDO2 — no special firmware needed.

Use a SoloKey to sign your Git commits with a hardware-backed **`ed25519-sk`** key (FIDO2). The private key never leaves the Solo, and every signature needs a **touch**. Commits show up as **Verified** on GitHub.

## 1. Create the key on the Solo
```bash
ssh-keygen -t ed25519-sk -O resident -O application=ssh:github \
           -C "$(whoami)@solo" -f ~/.ssh/id_ed25519_sk
```
Touch the Solo when it blinks (enter the Solo PIN first if you've set one).
- `-O resident` stores the credential on the key so it's recoverable on another machine with `ssh-keygen -K`. Drop it to skip using a credential slot.
- Add `-O verify-required` to also require the PIN on every use.

> **macOS:** Apple's bundled `ssh-keygen` has no FIDO support and fails with *"No FIDO SecurityKeyProvider specified"*. Use Homebrew's: `brew install openssh`, then `/opt/homebrew/bin/ssh-keygen` (and put `/opt/homebrew/bin` first on `PATH` so `ssh`/`git` use the FIDO-capable build).

## 2. Configure Git to sign with it
```bash
git config --global gpg.format ssh
git config --global user.signingkey ~/.ssh/id_ed25519_sk.pub
git config --global commit.gpgsign true
git config --global tag.gpgsign true
```

## 3. Add it to GitHub (for the "Verified" badge)
```bash
cat ~/.ssh/id_ed25519_sk.pub | pbcopy        # macOS; else just cat and copy
```
GitHub → **Settings → SSH and GPG keys → New SSH key → Key type = `Signing Key`** → paste → Add.

## 4. Sign a commit
```bash
git commit -m "signed with my Solo"          # touch the Solo when it blinks
```
Push to GitHub and the commit shows **Verified**.

## 5. Verify signatures locally (optional)
Tell Git which signers to trust, then `--show-signature` reports `Good "git" signature`:
```bash
mkdir -p ~/.config/git
echo "$(git config user.email) namespaces=\"git\" $(cat ~/.ssh/id_ed25519_sk.pub)" \
     > ~/.config/git/allowed_signers
git config --global gpg.ssh.allowedSignersFile ~/.config/git/allowed_signers
git log --show-signature -1
```

---

## Optional: use the same key for `git pull`/`push` (auth)
You *can* reuse this key for SSH authentication to GitHub (clone/pull/push over SSH), but it means a **touch on every Git network operation** — handy for security, mildly annoying day-to-day.

```bash
# add the SAME pubkey to GitHub a second time, as Key type = Authentication Key
# then use SSH remotes:
git remote set-url origin git@github.com:<you>/<repo>.git
```
Then `git push`/`pull` will prompt for a touch. If that's too much friction, leave auth on a normal key and keep the Solo just for **signing** (steps 1–4).
