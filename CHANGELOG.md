# v1.0.3 (unreleased)

This release fixes a FIDO authentication issue with Google.

## v1.0.3-rc.1 (2022-04-06)

### Bugfixes

- Correct the FIDO2 attestation certificate (fixes authentication issue with Google, [#36][])

[#36]: https://github.com/Nitrokey/nitrokey-3-firmware/issues/36

# v1.0.2 (2022-01-26)

This release improves compatibility with Windows systems.

## v1.0.2-rc.1 (2022-01-25)

Update to upstream release 1.0.9.

### Bugfixes

- usbd-ctaphid: fix ctaphid keepalive messages - fixes "busy" issue under Windows  ([#21][]) 

[#21]: https://github.com/Nitrokey/nitrokey-3-firmware/issues/21

# v1.0.1 (2022-01-15)

This release fixes some issues with the FIDO authenticator and the admin
application.

### Bugfixes

- fido-authenticator: use smaller CredentialID - fixes issues with some services FIDO usage ([fido-authenticator#8][])
- trussed: update P256 library - fixes signing failure in some cases ([#31][])

[#31]: https://github.com/Nitrokey/nitrokey-3-firmware/issues/31
[fido-authenticator#8]: https://github.com/solokeys/fido-authenticator/pull/8

## v1.0.1-rc.1 (2021-12-06)

### Features

- Change LED color and device name if provisioner app is enabled.

### Bugfixes

- admin-app: Fix CTAPHID command dispatch ([#8][]).
- admin-app: Fix CTAPHID wink command ([#9][]).
- fido-authenticator: Handle pin protocol field in hmac-secret extension data
  to fix the authenticatorGetAssertion command for newer clients ([#14][],
  [fido-authenticator#1][]).
- fido-authenticator: Signal credential protetection ([fido-authenticator#5][]).

[#8]: https://github.com/Nitrokey/nitrokey-3-firmware/issues/8
[#9]: https://github.com/Nitrokey/nitrokey-3-firmware/issues/9
[#14]: https://github.com/Nitrokey/nitrokey-3-firmware/issues/14
[fido-authenticator#1]: https://github.com/solokeys/fido-authenticator/pull/1
[fido-authenticator#5]: https://github.com/solokeys/fido-authenticator/pull/5

# v1.0.0 (2021-10-16)

First stable firmware release with FIDO authenticator.
