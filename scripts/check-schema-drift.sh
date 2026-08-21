#!/usr/bin/env bash
# Fails when the checked-in TypeScript does not match the Rust types.
#
# The wire contract is defined once, in the schema crate. The viewer compiles
# against generated TypeScript. A drifted type must fail the build, not the
# runtime, so this runs in continuous integration.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo test -p schema --quiet >/dev/null

if ! git diff --quiet -- client/src/gen; then
  echo "error: generated TypeScript is out of date with the Rust types."
  echo "run 'cargo test -p schema' and commit client/src/gen."
  git --no-pager diff -- client/src/gen
  exit 1
fi

echo "schema: generated TypeScript matches the Rust types."
