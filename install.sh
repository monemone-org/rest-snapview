#! /bin/bash

# Build a release binary
cargo build --release

# Copy it somewhere on your PATH
cp target/release/rest-snapview ~/scripts/



