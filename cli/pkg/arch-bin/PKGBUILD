# Maintainer: Nicolas Stalder <n+archlinux@stalder.io>
# Helpful suggestions by Foxboron
pkgname=solo2-cli-bin
pkgver=0.2.1
pkgrel=1
pkgdesc='Solo 2 CLI'
arch=('x86_64')
url="https://github.com/solokeys/solo2-cli"
license=(Apache MIT)
# we only need `libudev.so`, during build we also need `pkgconfig/udev/.pc`
depends=(systemd-libs ccid)
# note we do not need Arch `hidapi` package here, it's a git submodule of Rust hidapi
makedepends=(git systemd)
provides=(solo2-cli)
conflicts=(solo2-cli)

source=(
  "solo2::${url}/releases/download/v${pkgver}/solo2-v${pkgver}-x86_64-unknown-linux-gnu"
  "70-solo2.rules::${url}/releases/download/v${pkgver}/70-solo2.rules"
  "solo2.bash::${url}/releases/download/v${pkgver}/solo2.completions.bash"
  "solo2.zsh::${url}/releases/download/v${pkgver}/solo2.completions.zsh"
  "LICENSE-MIT::${url}/raw/v${pkgver}/LICENSE-MIT")
sha256sums=('3bede0161a0d9da51d961b6c1c46af2da7defaf450dd42ce7ed57a8b8bb0b2a4'
            '4133644b12a4e938f04e19e3059f9aec08f1c36b1b33b2f729b5815c88099fe3'
            'a892afc3c71eb09c1d8e57745dabbbe415f6cfd3f8b49ee6084518a07b73d9a8'
            '70bd6aa5ebfb2ec67b12f546d34af9cfe2ffe92e0366c44c9ce0633d0582ebf3'
            'bdc889204ff84470aaad9f6fc66829cd1cdfb78b307fe3a8c0fe7be5353e1165')

package() {
  install -Dm755 solo2 "$pkgdir/usr/bin/solo2"
  install -Dm644 LICENSE-MIT "$pkgdir/usr/share/licenses/$pkgnamefull/LICENSE-MIT"

  # completions
  install -Dm644 solo2.zsh "$pkgdir/usr/share/zsh/site-functions/_solo2"
  install -Dm644 solo2.bash "$pkgdir/usr/share/bash-completion/completions/solo2"

  # udev rule
  install -Dm644 70-solo2.rules -t "$pkgdir/usr/lib/udev/rules.d"
}
