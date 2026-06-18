# solo2 on NXP LPC55S69

### the open source FIDO2 security key — built with Trussed®

<a href="https://solokeys.com"><img src="https://solokeys.com/cdn/shop/files/USBCHacker_600x600.jpg?v=1682534858" alt="Solo 2C Hacker" height="220"></a>
&nbsp;&nbsp;
<a href="https://www.nxp.com/products/processors-and-microcontrollers/arm-based-processors-and-mcus/lpc-cortex-m-mcus/lpc5500-cortex-m33/lpcxpresso55s69-development-board:LPC55S69-EVK"><img src="https://compoindia.com/wp-content/uploads/2022/09/LPC55S69-EVK__56222.jpg" alt="LPCXpresso55S69-EVK" height="220"></a>

This is the firmware for the **LPC55S69** — the NXP Cortex-M33 silicon inside
the shipping **[Solo 2](https://solokeys.com)**. It runs on two boards:

- **Solo 2 Hacker** — the real SoloKeys device, unlocked for developers
  (PRINCE-encrypted storage, no debug port). Get one from
  [solokeys.com](https://solokeys.com).
- **LPCXpresso55S69-EVK** — NXP's evaluation board with the same chip and an
  onboard J-Link, for recoverable development. Get one from
  [nxp.com](https://www.nxp.com/products/processors-and-microcontrollers/arm-based-processors-and-mcus/lpc-cortex-m-mcus/lpc5500-cortex-m33/lpcxpresso55s69-development-board:LPC55S69-EVK).

## Build

```
make build-dev      # EVK / development build (plain storage)
make build-release  # Solo 2 Hacker, PRINCE-encrypted (provisioned devices)
```
