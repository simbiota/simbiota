profile ?= debug

# calculated stuff

cargo_profile :=
ifeq (release, $(profile))
cargo_profile := --release
endif

.PHONY: build-pi4-armv7
build-pi4-armv7:
	cargo build $(cargo_profile) --target=armv7-unknown-linux-gnueabihf

.PHONY: build-pi4-arm64
build-pi4-arm64:
	CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc cargo build $(cargo_profile) --target=aarch64-unknown-linux-gnu

.PHONY: pi
pi: build-pi4-armv7

ifeq (run,$(firstword $(MAKECMDGOALS)))
  # use the rest as arguments for "run"
  RUN_ARGS := $(wordlist 2,$(words $(MAKECMDGOALS)),$(MAKECMDGOALS))
  # ...and turn them into do-nothing targets
  $(eval $(RUN_ARGS):;@:)
endif

ifeq (debug,$(firstword $(MAKECMDGOALS)))
  # use the rest as arguments for "run"
  RUN_ARGS := $(wordlist 2,$(words $(MAKECMDGOALS)),$(MAKECMDGOALS))
  # ...and turn them into do-nothing targets
  $(eval $(RUN_ARGS):;@:)
endif

# Packaging
.PHONY: completions
completions:
	mkdir -p ./package/common/usr/share/bash-completion/completions
	mkdir -p ./package/common/usr/share/zsh/vendor-completions
	mkdir -p ./package/common/usr/share/fish/completions
	find ./target -name "simbiotactl.bash" -exec cp "{}" ./package/common/usr/share/bash-completion/completions/simbiotactl \;
	find ./target -name "_simbiotactl" -exec cp "{}" ./package/common/usr/share/zsh/vendor-completions/_simbiotactl \;
	find ./target -name "simbiotactl.fish" -exec cp "{}" ./package/common/usr/share/fish/completions \;

# man pages
.PHONY: man-pages
man-pages:
	sphinx-build -b man man man-out
	rm -r man-out/.doctrees
	mkdir -p ./package/common/usr/share/man/man5/
	mkdir -p ./package/common/usr/share/man/man8/
	cat man-out/simbiota_config.5 | gzip > ./package/common/usr/share/man/man5/simbiota_config.5.gz
	cat man-out/simbiota.8 | gzip > ./package/common/usr/share/man/man8/simbiota.8.gz

.PHONY: deb-pi4-armv7 deb-pi4-armv7-dep deb-pi4-arm64 deb-pi4-armv64-dep deb-common deb-pi4-armv7-nodep deb-pi4-arm64-nodep
deb-pi4-armv7-dep: build-pi4-armv7
deb-pi4-arm64-dep: build-pi4-arm64

deb-common: completions man-pages

deb-pi4-armv7: deb-pi4-armv7-dep deb-pi4-armv7-nodep
deb-pi4-armv7-nodep: deb-common
	cp -R ./package/common/* ./package/deb-pi4-armv7/
	rm ./package/deb-pi4-armv7/var/lib/simbiota/.gitkeep
	mkdir -p ./package/deb-pi4-armv7/usr/sbin
	mkdir -p ./package/deb-pi4-armv7/etc/simbiota

	@-rm ./package/deb-pi4-armv7/usr/sbin/simbiota
	@-rm ./package/deb-pi4-armv7/usr/sbin/simbiotactl
	cp target/armv7-unknown-linux-gnueabihf/$(profile)/simbiota ./package/deb-pi4-armv7/usr/sbin/simbiota
	cp target/armv7-unknown-linux-gnueabihf/$(profile)/simbiotactl ./package/deb-pi4-armv7/usr/sbin/simbiotactl


	ls -lahR package/deb-pi4-armv7
	chmod 0755 package/deb-pi4-armv7/DEBIAN
	chmod 0755 package/deb-pi4-armv7/usr/sbin/simbiota
	chmod 0755 package/deb-pi4-armv7/usr/sbin/simbiotactl
	chmod 0644 package/deb-pi4-armv7/etc/simbiota/client.yaml
	fakeroot dpkg-deb -Zxz --root-owner-group --build package/deb-pi4-armv7
	mv package/deb-pi4-armv7.deb package/simbiota_armv7.deb

deb-pi4-arm64: deb-pi4-arm64-dep deb-pi4-arm64-nodep
deb-pi4-arm64-nodep: deb-common
	cp -R ./package/common/* ./package/deb-pi4-arm64/
	rm ./package/deb-pi4-arm64/var/lib/simbiota/.gitkeep
	mkdir -p ./package/deb-pi4-arm64/usr/sbin
	mkdir -p ./package/deb-pi4-arm64/etc/simbiota

	@-rm ./package/deb-pi4-arm64/usr/sbin/simbiota
	@-rm ./package/deb-pi4-arm64/usr/sbin/simbiotactl
	cp target/aarch64-unknown-linux-gnu/$(profile)/simbiota ./package/deb-pi4-arm64/usr/sbin/simbiota
	cp target/aarch64-unknown-linux-gnu/$(profile)/simbiotactl ./package/deb-pi4-arm64/usr/sbin/simbiotactl

	ls -lahR package/deb-pi4-arm64
	chmod 0755 package/deb-pi4-arm64/DEBIAN
	chmod 0755 package/deb-pi4-arm64/usr/sbin/simbiota
	chmod 0755 package/deb-pi4-arm64/usr/sbin/simbiotactl
	chmod 0644 package/deb-pi4-arm64/etc/simbiota/client.yaml
	fakeroot dpkg-deb -Zxz --root-owner-group --build package/deb-pi4-arm64
	mv package/deb-pi4-arm64.deb package/simbiota_arm64.deb

.PHONY: all clean package package-nodep
package: deb-pi4-armv7 deb-pi4-arm64
package-nodep: deb-pi4-armv7-nodep deb-pi4-arm64-nodep
all: pi
clean:
	cargo clean
	@-rm -rf ./package/deb-pi4-armv7/usr ./package/deb-pi4-armv7/etc ./package/deb-pi4-armv7/DEBIAN/postinst ./package/deb-pi4-armv7/DEBIAN/prerm ./package/deb-pi4-armv7/DEBIAN/conffiles
	@-rm -rf ./package/deb-pi4-arm64/usr ./package/deb-pi4-armv64/etc ./package/deb-pi4-armv64/DEBIAN/postinst ./package/deb-pi4-armv64/DEBIAN/prerm ./package/deb-pi4-armv64/DEBIAN/conffiles
	@-rm -rf ./package/*.deb
	@-rm -rf ./man-out
	@-rm -rf ./package/common/usr/share