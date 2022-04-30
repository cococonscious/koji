#!/usr/bin/env bash
set -euo pipefail

export KOJI_HOME="$(cd "$(dirname "$0")/.." && pwd)"

echoerr() {
   echo "$@" 1>&2
}

release() {
   TAR_DIR="${KOJI_HOME}/target/tar"

   target="${1:-}"
   if [[ $target == *"osx"* ]]; then
      echoerr "OSX cross-compile is impossible. Fallbacking to cargo..."
      target=""
   fi

   cd "$KOJI_HOME"

   rm -rf "${KOJI_HOME}/target" 2> /dev/null || true

   if [ -n "$target" ]; then
      cargo install --version 0.2.1 cross 2> /dev/null || true
      cross build --release --target "$target"
      bin_folder="${target}/release"
   else
      cargo build --release
      bin_folder="release"
   fi

   koji_bin_path="${KOJI_HOME}/target/${bin_folder}/koji"
   chmod +x "$koji_bin_path"
   mkdir -p "$TAR_DIR" 2> /dev/null || true

   cp "$koji_bin_path" "$TAR_DIR"
   cp "$KOJI_HOME/LICENSE" "$TAR_DIR"

   cd "$TAR_DIR"
   tar -czf koji.tar.gz *
}

cmd="$1"
shift

release "$@"
