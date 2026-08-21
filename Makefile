PKG     := powpow
VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ARCH    := amd64
DEB     := $(PKG)_$(VERSION)_$(ARCH).deb

# Docker image used for the build — Rust on Debian 13 (trixie)
RUST_IMAGE := rust:1.93-trixie

.PHONY: deb clean

deb: $(DEB)

$(DEB): target/release-trixie/powpow $(shell find pkg -type f)
	rm -rf _deb
	mkdir -p _deb/DEBIAN _deb/usr/sbin _deb/etc _deb/lib/systemd/system
	sed 's/@@PKG@@/$(PKG)/;s/@@VERSION@@/$(VERSION)/;s/@@ARCH@@/$(ARCH)/' pkg/control.in > _deb/DEBIAN/control
	echo /etc/powpow.conf > _deb/DEBIAN/conffiles
	cp pkg/postinst pkg/prerm pkg/postrm _deb/DEBIAN/
	chmod 755 _deb/DEBIAN/postinst _deb/DEBIAN/prerm _deb/DEBIAN/postrm
	cp target/release-trixie/powpow _deb/usr/sbin/powpow
	chmod 755 _deb/usr/sbin/powpow
	cp .env.example _deb/etc/powpow.conf
	chmod 640 _deb/etc/powpow.conf
	cp pkg/powpow.service _deb/lib/systemd/system/powpow.service
	chmod 644 _deb/lib/systemd/system/powpow.service
	dpkg-deb --build --root-owner-group _deb $(DEB)
	rm -rf _deb
	@echo "Built $(DEB)"

target/release-trixie/powpow: Cargo.toml Cargo.lock $(shell find src -type f) $(shell find migrations -type f)
	mkdir -p target/release-trixie
	docker run --rm \
		-v "$(CURDIR)":/build \
		-w /build \
		$(RUST_IMAGE) \
		sh -c 'apt-get update && apt-get install -y cmake && cargo build --release && cp target/release/powpow target/release-trixie/powpow'

clean:
	rm -rf _deb target/release-trixie $(PKG)_*.deb
