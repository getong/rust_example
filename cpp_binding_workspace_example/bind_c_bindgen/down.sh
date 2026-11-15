#!/bin/sh

set -eu
cd "$(dirname "$0")"

if [ -f SDL3-3.4.12/CMakeLists.txt ]; then
  exit 0
fi

curl -fL --connect-timeout 30 -o SDL3-3.4.12.tar.gz \
  https://www.libsdl.org/release/SDL3-3.4.12.tar.gz \
  && tar xzf SDL3-3.4.12.tar.gz
