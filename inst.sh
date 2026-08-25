#!/bin/bash
# First run: cargo build --release
test -f target/release/ebba && cp target/release/ebba ~/.local/bin/ebba || echo "No target binary. Did you run cargo build --release?"
