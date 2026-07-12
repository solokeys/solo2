# Security Policy

Solo 2 is a security product. We take vulnerabilities seriously and appreciate responsible disclosure.

## Reporting a vulnerability

**Please do not open public GitHub issues or pull requests for security vulnerabilities.**

Report privately by either:

- [GitHub private vulnerability reporting](https://github.com/solokeys/solo2/security/advisories/new) (preferred), or
- Email **security@solokeys.com** with subject `SECURITY: <short summary>`.

Please include:

- affected component (firmware app, CLI, a Trussed crate, hardware) and version (`solo2 app admin version`),
- a description and, if possible, steps to reproduce or a proof of concept,
- the impact you believe it has (e.g. credential disclosure, bypass of user presence/PIN, secure-boot bypass).

We aim to acknowledge reports within a few business days and will keep you updated on remediation. We're happy to credit reporters in the release notes unless you prefer otherwise.

We do **not** currently run a paid bug bounty program.

## Scope

**In scope** — issues in this repository and the firmware it builds:

- the apps (FIDO2, OATH, PIV, OpenPGP, admin/NDEF) and the runner,
- the `solo2` CLI in [`cli/`](cli/),
- secure boot / firmware signing / firmware lock state weaknesses on **Secure** keys,
- cryptographic flaws, user presence or PIN bypasses, memory safety issues.

**Out of scope:**

- **Hacker keys.** Hacker keys are intentionally unlocked and run arbitrary, unsigned firmware. Findings that rely on a Hacker's unlocked state (custom/unsigned firmware, no secure boot seal, debug access) are **by design, not vulnerabilities** — that's the difference from the locked Secure model.
- Physical attacks requiring destructive decapsulation, or attacks already mitigated by the documented threat model.
- Vulnerabilities in third-party host tools we merely interoperate with (report those upstream).

## Security model (summary)

- **Secure** keys are locked/sealed: secure boot is enforced and debug access is disabled, so only firmware signed by us can run. `solo2 update` only installs signed releases.
- **Hacker** keys are unlocked so owners can build and flash their own firmware; this removes the secure boot guarantee by design.
- Sensitive operations require **user presence** (touch) and, where applicable, a **PIN**.

## Supported versions

Security fixes target the **latest released firmware**. Please verify against the latest `solo2 update` build before reporting.
