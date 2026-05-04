set working-directory := "api"

run:
    cargo run

test:
    cargo test

fmt:
    cargo fmt
    cargo clippy

go:
    cargo test
    cargo fmt
    cargo clippy
