# Personal PKGBUILD - not for AUR submission.
# Clones the rust branch, builds, installs locally.
# Usage: makepkg -si

pkgname=markerss
pkgver=0.1.0.r97.99fdda0
pkgrel=1
pkgdesc="TUI RSS reader - browse feeds in the terminal, store blog posts as markdown on command"
arch=('x86_64')
url="https://github.com/HiraethEcho/markerss"
license=('custom') # repo has no LICENSE file yet
depends=('ca-certificates') # rustls-native-certs: needs system CA bundle at runtime
makedepends=('cargo' 'git')
source=("$pkgname::git+https://github.com/HiraethEcho/markerss.git#branch=rust")
sha256sums=('SKIP')

pkgver() {
  cd "$srcdir/$pkgname"
  printf '0.1.0.r%s.%s' "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

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
