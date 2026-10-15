#!/usr/bin/env bash
set -euo pipefail

ACTION="${1:-Dev}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
SYSROOT="${HOME}/.cargo/sysroot"
if [ -d "$SYSROOT/usr/lib64/pkgconfig" ]; then
    export PKG_CONFIG_PATH="$SYSROOT/usr/lib64/pkgconfig:${PKG_CONFIG_PATH:-}"
    export LIBRARY_PATH="$SYSROOT/usr/lib64:${LIBRARY_PATH:-}"
    export LD_LIBRARY_PATH="$SYSROOT/usr/lib64:${LD_LIBRARY_PATH:-}"
fi

if [ -z "${CARGO_BUILD_JOBS:-}" ]; then
    export CARGO_BUILD_JOBS="2"
fi

cd "$PROJECT_ROOT"

case "$ACTION" in
    Validate)
        SMOKE_DIR="$PROJECT_ROOT/output/toolchain"
        mkdir -p "$SMOKE_DIR"
        echo 'fn main() { println!("soundflow-rust-ready"); }' > "$SMOKE_DIR/smoke.rs"
        rustc "$SMOKE_DIR/smoke.rs" -o "$SMOKE_DIR/smoke"
        "$SMOKE_DIR/smoke"
        ;;
    Check)
        cargo check --locked --manifest-path Cargo.toml
        ;;
    Test)
        cargo test --release --locked --manifest-path Cargo.toml
        ;;
    Build)
        cargo build --release --locked --manifest-path Cargo.toml
        ;;
    Dev)
        cargo run --locked --manifest-path Cargo.toml
        ;;
    *)
        echo "Unknown action: $ACTION. Supported: Dev, Build, Check, Test, Validate"
        exit 1
        ;;
esac
