NAME:=net-ifnames-prefix
MAJOR:=0
MINOR:=1
PATCH:=0

VERSION:=$(MAJOR).$(MINOR).$(PATCH)
ARCHIVE:=$(NAME)-$(VERSION).tar.xz

FEDORA_VERSION:=rawhide

all: debug

debug: target/debug/$(NAME)
	@cargo build

release: target/release/$(NAME)
	@cargo build --release

.PHONY: check dist srpm rpm clean

check:
	@cargo test

dist:
	@git archive HEAD --prefix net-ifnames-prefix-0.1.0/ | xz > $(ARCHIVE)

srpm: dist
	@rpmbuild -ts $(ARCHIVE)

rpm: srpm
	@rpmbuild -tb $(ARCHIVE)

mock-rpm: srpm
	$(eval SRPM:=$(shell ls ~/rpmbuild/SRPMS/$(NAME)*.src.rpm))
	@mock -r fedora-$(FEDORA_VERSION)-x86_64 --rebuild $(SRPM)

clean:
	@cargo clean
	@rm -f $(ARCHIVE)
