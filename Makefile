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

.PHONY: check dist srpm rpm clean

check:
	@cargo test

dist:
	@git archive HEAD --prefix net-ifnames-prefix-0.1.0/ | xz > $(ARCHIVE)

srpm: dist
	@rpmbuild -ts $(ARCHIVE)

rpm: srpm
	@rpmbuild -tb $(ARCHIVE)

clean:
	@cargo clean
	@rm -f $(ARCHIVE)
