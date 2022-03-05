.PHONY: build

build:
	cargo fmt -- --check
	cargo clippy --locked -- -D warnings
	cargo build --locked
	cargo test --locked
	# This can fail when cargo build succeeds so we need to make sure it's working.
	cargo install --path . --force
