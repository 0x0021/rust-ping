#!/usr/bin/env bash
# 用法: build-pkg.sh <version> <binary-path> <output.pkg>
set -euo pipefail

VERSION="$1"
BIN="$2"
OUT="$3"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

# 避免 macOS 在暂存目录生成 ._ AppleDouble 伴随文件
export COPYFILE_DISABLE=1

mkdir -p "$STAGE/usr/local/bin" "$STAGE/usr/local/share/doc/rust-ping"
cp "$BIN" "$STAGE/usr/local/bin/rust-ping"
cp README.md config.example.toml assets/icon.png "$STAGE/usr/local/share/doc/rust-ping/"
find "$STAGE" -name '._*' -delete

pkgbuild \
  --root "$STAGE" \
  --identifier com.0x0021.rust-ping \
  --version "$VERSION" \
  --install-location / \
  "$OUT"

echo "已生成: $OUT"
