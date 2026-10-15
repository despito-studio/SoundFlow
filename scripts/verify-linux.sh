#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
SYSROOT="${HOME}/.cargo/sysroot"
if [ -d "$SYSROOT/usr/lib64/pkgconfig" ]; then
    export PKG_CONFIG_PATH="$SYSROOT/usr/lib64/pkgconfig:${PKG_CONFIG_PATH:-}"
    export LIBRARY_PATH="$SYSROOT/usr/lib64:${LIBRARY_PATH:-}"
    export LD_LIBRARY_PATH="$SYSROOT/usr/lib64:${LD_LIBRARY_PATH:-}"
fi

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-$PROJECT_ROOT/output/native}"
mkdir -p "$OUTPUT_DIR"

echo "=== 1. Running Rust unit tests ==="
cargo test --release --locked --manifest-path "$PROJECT_ROOT/Cargo.toml"

echo "=== 2. Running media-provider tests ==="
(cd "$PROJECT_ROOT/media-provider" && npm test)

echo "=== 3. Running native verification suite ==="
REPORT_FILE="$OUTPUT_DIR/native-report.json"
"$PROJECT_ROOT/target/release/soundflow" --verify-native "$REPORT_FILE" --resources "$PROJECT_ROOT"

cat "$REPORT_FILE"
echo ""
echo "=== Linux verification complete and passed! ==="
