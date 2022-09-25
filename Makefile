# https://tech.davis-hansson.com/p/make/
SHELL := bash
.ONESHELL:
.SHELLFLAGS := -eu -o pipefail -c
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

ifeq ($(origin .RECIPEPREFIX), undefined)
  $(error This Make does not support .RECIPEPREFIX. Please use GNU Make 4.0 or later)
endif
.RECIPEPREFIX = >

APP := git-drive

DESTDIR ?=
PREFIX  ?= /usr/local
CARGOFLAGS ?=


# generate release build
all: build
build: target/release/$(APP)

# install release build to local cargo bin directory
install: $(DESTDIR)$(PREFIX)/bin/$(APP)

# Remove installed binary
uninstall:
> -rm -- "$(DESTDIR)$(PREFIX)/bin/$(APP)"

# development builds
check: target/debug/$(APP)
test: .cargoinstalled
> cargo test --all --all-targets --all-features

# clean build output
clean: .cargoinstalled
> cargo clean

.PHONY: all build clean install uninstall check test

### build targets

target/debug/$(APP): .cargoinstalled Cargo.toml Cargo.lock $(shell find src -type f)
> cargo build --bin $(APP)

target/release/$(APP): .cargoinstalled Cargo.toml Cargo.lock $(shell find src -type f)
> RUSTFLAGS="-C link-arg=-s -C target-cpu=native" cargo build $(CARGOFLAGS) --bin $(APP) --release

$(DESTDIR)$(PREFIX)/bin/$(APP): target/release/$(APP)
> install -m755 -- target/release/$(APP) "$(DESTDIR)$(PREFIX)/bin/"

.cargoinstalled:
> @if ! command -v cargo 2> /dev/null
> @then
>   @echo "Cargo is not installed. Please visit 'https://rustup.rs/' and follow their instructions, or try to run 'curl --proto \"=https\" --tlsv1.2 -sSf https://sh.rustup.rs | sh'"
>   @exit 1
> @fi
> touch .cargoinstalled
