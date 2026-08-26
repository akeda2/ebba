#!/bin/bash
# First run: cargo build --release
cargo install --path .
test -f target/release/ebba && sudo cp target/release/ebba /usr/local/bin/ebba || echo "No target binary. Did you run cargo build --release?"
which ebba
sudo which ebba
which gb && gb -I ebba.yaml -a -w
