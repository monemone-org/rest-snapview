#! /bin/bash

# Build a release binary
cargo build --release

# Copy it somewhere on your PATH
sudo cp target/release/rest-snapview /usr/local/bin/
