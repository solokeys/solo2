# Skills

Reusable workflows for agents working with Solo 2. Each skill is a directory with a `SKILL.md` (frontmatter `name` + `description`, then instructions). See [AGENTS.md](../AGENTS.md) for the bigger picture.

## Available

- **[solo2-cli](solo2-cli)** — install, run, and understand the `solo2` CLI (list, update, app subcommands).
- **[solo2-examples](solo2-examples)** — find the right tested tutorial in `examples/` (passkeys, SSH, git, age, OATH, PIV/OpenPGP, post-quantum) and which model it works on.
- **[flash-solo-hacker](flash-solo-hacker)** — build, verify, and flash Hacker firmware safely: lock-state/storage checks, EVK-first validation, brick-avoidance.

## Ideas for more skills

Candidates worth adding as the project grows:

- **flash-evk** — the recoverable dev-board loop (build → J-Link flash → run `runners/pc` tests against the board); the safe sandbox the other flashing skills point to.
- **debug-brick** — triage a non-enumerating key (button-hold replug, bootloader recovery, `lpc55 pfr` reads) before declaring it bricked.
- **fido-conformance** — run the FIDO Alliance conformance suite against the EVK.
- **nfc-debug** — bring up and debug FIDO2/NDEF over NFC (reader setup, short vs extended APDUs).
