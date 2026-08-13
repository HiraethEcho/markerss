# Personal PKGBUILD - not for AUR submission.
# Builds from the v0.1.0 tag of the rust branch.
# Usage: makepkg -si

pkgname=markerss
pkgver=0.1.0
pkgrel=1
pkgdesc="TUI RSS reader - browse feeds in the terminal, store blog posts as markdown on command"
arch=('x86_64')
url="https://github.com/HiraethEcho/markerss"
license=('custom') # repo has no LICENSE file yet
depends=('ca-certificates') # rustls-native-certs: needs system CA bundle at runtime
makedepends=('cargo' 'git')
source=("$pkgname::git+https://github.com/HiraethEcho/markerss.git#tag=v0.1.0")
sha256sums=('SKIP')

build() {
  cd "$srcdir/$pkgname"
  # makepkg CFLAGS (-fno-plt etc.) break aws-lc-sys link: undefined aws_lc_*_EVP_PKEY_free symbols
  export CFLAGS="" CXXFLAGS=""
  cargo build --release --locked
}

package() {
  cd "$srcdir/$pkgname"
  install -Dm755 target/release/markerss "$pkgdir/usr/bin/markerss"
}
