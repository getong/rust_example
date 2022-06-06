#!/usr/bin/env bash

set -e

_DIR=$(dirname $(realpath "$0"))

cd $_DIR

if [ ! -e "test.pdf" ]; then
wget "https://netc.jnu.edu.cn/_upload/article/files/e3/f1/996a071a4fec800c131898d4ec57/98e9cc0f-12a8-4d93-98e8-13f7fad3476a.pdf" -O test.pdf
fi

RUST_BACKTRACE=1 cargo +nightly build --release

./target/release/blake3_merkle_example > main.out 2>&1

cat main.out
