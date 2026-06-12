# Maintainer: Nicolas Stalder <n+archlinux@stalder.io>
# Helpful suggestions by Foxboron
pkgname=solo2-cli
pkgver=0.2.1
pkgrel=1
pkgdesc='Solo 2 CLI'
arch=('x86_64')
url="https://github.com/solokeys/solo2-cli"
license=(Apache MIT)
# we only need `libudev.so`, during build we also need `pkgconfig/udev/.pc`
depends=(systemd-libs ccid)
# note we do not need Arch `hidapi` package here, it's a git submodule of Rust hidapi
makedepends=(cargo git systemd)
source=(
	"$pkgname-$pkgver.tar.gz::https://github.com/solokeys/solo2-cli/archive/refs/tags/v${pkgver}.tar.gz"
)
sha256sums=(
    "ae9ef9dd174a8b8294941635a3a66dc9062fd4b595d5f1f6507b5a5a232d6932"
)

prepare() {
  cd "${pkgname}-${pkgver}"
  cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
  cd "${pkgname}-${pkgver}"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  # dev-pki feature not activated due to bug in v0.1.1
  # cargo build --release --frozen --all-features
  cargo build --features dev-pki --release --frozen
}

check() {
  cd "${pkgname}-${pkgver}"
  export RUSTUP_TOOLCHAIN=stable
  # make sure shared libs work
  target/release/solo2 --version
  # dev-pki feature not activated due to bug in v0.1.1
  # cargo test --release --frozen --all-features
  cargo test --features dev-pki --release --frozen
}

package() {
  cd "${pkgname}-${pkgver}"
  install -Dm755 target/release/solo2 "$pkgdir/usr/bin/solo2"
  install -Dm644 LICENSE-MIT "$pkgdir/usr/share/licenses/$pkgnamefull/LICENSE-MIT"

  # completions
  install -Dm644 target/release/_solo2 -t "$pkgdir/usr/share/zsh/site-functions"
  install -Dm644 target/release/solo2.bash "$pkgdir/usr/share/bash-completion/completions/solo2"

  # udev rule
  install -Dm644 70-solo2.rules -t "$pkgdir/usr/lib/udev/rules.d"
}
