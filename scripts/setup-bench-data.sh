#!/usr/bin/env bash
set -euo pipefail

BENCH_DIR="target/bench-data/rust"

if [ -d "$BENCH_DIR" ]; then
    echo "Dataset already exists at $BENCH_DIR"
else
    echo "Cloning rust-lang/rust (shallow) to $BENCH_DIR..."
    mkdir -p target/bench-data
    git clone --depth 1 --single-branch https://github.com/rust-lang/rust.git "$BENCH_DIR"
fi

FILE_COUNT=$(find "$BENCH_DIR" -name "*.md" | wc -l)
echo "Dataset ready: $FILE_COUNT Markdown files"
