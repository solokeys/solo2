# Maintainer: Nicolas Stalder <n+archlinux@stalder.io>
pkgname=solo2-cli-git
pkgver=r66.dd05f51
pkgrel=1
pkgdesc="Command line interface to SoloKeys Solo 2 devices"
url="https://github.com/solokeys/solo2-cli"
arch=(any)
license=(MIT)
depends=(systemd-libs ccid)
makedepends=(cargo git systemd)
provides=(solo2-cli)
conflicts=(solo2-cli)

source=('git+https://github.com/solokeys/solo2-cli.git#branch=main')
# add dummy entries for `make generate-checksums` to create SHA256 instead of MD5 check sums
sha256sums=('SKIP')

pkgver() {
  cd "$srcdir/solo2-cli"
  printf "r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

prepare() {
  cd $srcdir/solo2-cli
  cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
  cd $srcdir/solo2-cli
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  cargo build --release --frozen --all-features
}

check() {
  cd $srcdir/solo2-cli
  export RUSTUP_TOOLCHAIN=stable
  # make sure shared libs work
  target/release/solo2 --version
  cargo test --release --all-features
}

package() {
  install -Dm755 "$srcdir/solo2-cli/target/release/solo2" "$pkgdir/usr/bin/solo2"
  install -Dm644 "$srcdir/solo2-cli/LICENSE-MIT" "$pkgdir/usr/share/licenses/$pkgname/LICENSE-MIT"

  # completions
  install -Dm644 $srcdir/solo2-cli/target/release/_solo2 -t "$pkgdir/usr/share/zsh/site-functions"
  install -Dm644 $srcdir/solo2-cli/target/release/solo2.bash "$pkgdir/usr/share/bash-completion/completions/solo2"

  # udev rule
  install -Dm644 $srcdir/solo2-cli/70-solo2.rules -t "$pkgdir/usr/lib/udev/rules.d"
}
