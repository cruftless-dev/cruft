#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEST="${1:-$ROOT/dist/standalone}"
CRUFT_BIN="${2:-$ROOT/target/release/cruft}"

[ -f "$CRUFT_BIN" ] || {
    echo "stage-standalone: cruft binary not found at $CRUFT_BIN" >&2
    echo "  build it first with: cargo build --release -p cruft --bin cruft" >&2
    exit 1
}

mkdir -p "$DEST/bin"
cp "$CRUFT_BIN" "$DEST/bin/cruft"
chmod +x "$DEST/bin/cruft"
cat >"$DEST/bin/cpx" <<'EOF'
#!/usr/bin/env sh
set -eu
self=$0
case "$self" in
  */*) dir=${self%/*} ;;
  *) dir=. ;;
esac
exec "$dir/cruft" exec "$@"
EOF
chmod +x "$DEST/bin/cpx"

echo "stage-standalone: staged $DEST"
echo "  native payload: bin/cruft"
echo "  cpx shim:       bin/cpx -> cruft exec"
