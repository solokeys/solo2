# Raspberry Pi 4B

The intent of this platform is to be a driver for CI, that is, run a self-hosted GitHub Actions runner.

The firmware would then be compiled on GitHub's usual Ubuntu CI servers, and just the built artifact
transfered for functional HIL testing.

We use Arch Linux for its wide collection of up-to-date packages, and the ease of building one's one
(e.g., we'll want to package up our own `lpc55-host`).

- [Install Arch on a MicroSD card](https://archlinuxarm.org/platforms/armv8/broadcom/raspberry-pi-4) 
(AArch64 works fine, don't forget the last `sed` step, make sure you're not in the RPi3 section).
- If network doesn't come up reliably (`networkctl` shows `eth0` as `Configuring`), you can fix manually
with `networkctl down eth0`, `networkctl up eth0`, but 
[this change to mkinitcpio.conf](https://github.com/raspberrypi/linux/issues/3108#issuecomment-723580334)
also seems to work (early `systemd-networkd` journal entries indicate `eth0` can't be found).
- No need to try to compile `ncurses5` to `arm-none-eabi-gdb` from <https://developer.arm.com/> to work: just use `gdb`!
- For `jlink-software-and-documentation`, if it's not merged yet, apply a patch like 
<https://aur.archlinux.org/packages/jlink-software-and-documentation#comment-786082>. It seems like not
having a GUI suppresses `JLinkGDBServer`'s pop-up and confirmation attempts.

## The Runner
Install it as described in the Settings > Actions tab, then keep it running with:

```
[Unit]
Description=GitHub Actions runner

[Service]
Type=simple
ExecStart=/home/alarm/actions-runner/run.sh
WorkingDirectory=/home/alarm/actions-runner

[Install]
WantedBy=default.target
```
