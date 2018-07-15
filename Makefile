NAME:=net-ifnames-prefix
MAJOR:=0
MINOR:=1
PATCH:=0

VERSION:=$(MAJOR).$(MINOR).$(PATCH)
ARCHIVE:=$(NAME)-$(VERSION).tar.xz

all: debug

debug: target/debug/$(NAME)
	@cargo build

release: target/release/$(NAME)
	@cargo build --release

.PHONY: check dist srpm rpm

check:
	@cargo test

dist:
	@tar -cJf $(ARCHIVE) src udev redhat tests Cargo.toml LICENSE README.md

srpm: dist
	@rpmbuild -ts $(ARCHIVE)

rpm: srpm
	@rpmbuild -tb $(ARCHIVE)
