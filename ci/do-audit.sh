#!/usr/bin/env bash

set -e

here="$(dirname "$0")"
src_root="$(readlink -f "${here}/..")"

cd "${src_root}"

# `cargo-audit` doesn't give us a way to do this nicely, so hammer it is...
dep_tree_filter="grep -Ev '│|└|├|─'"

while [[ -n $1 ]]; do
  if [[ $1 = "--display-dependency-trees" ]]; then
    dep_tree_filter="cat"
    shift
  fi
done

cargo_audit_ignores=(
  # Potential segfault in the time crate
  #
  # Blocked on chrono updating `time` to >= 0.2.23
  --ignore RUSTSEC-2020-0071

  # tokio: vulnerability affecting named pipes on Windows
  #
  # Exception is a stopgap to unblock CI
  # https://github.com/solana-labs/solana/issues/29586
  --ignore RUSTSEC-2023-0001

  --ignore RUSTSEC-2022-0093

  # mio
  --ignore RUSTSEC-2024-0019

  # curve25519-dalek
  # Patches to address the advisory have been pulled into a fork of the repo.
  # See `[patch.crates-io.curve25519-dalek]` in Cargo.toml for more information
  --ignore RUSTSEC-2024-0344

  # openssl
  --ignore RUSTSEC-2024-0357
)
scripts/cargo-for-all-lock-files.sh audit "${cargo_audit_ignores[@]}" | $dep_tree_filter
# we want the `cargo audit` exit code, not `$dep_tree_filter`'s
exit "${PIPESTATUS[0]}"
