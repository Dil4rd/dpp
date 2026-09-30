#!/usr/bin/env bash
#
# Publishes every crates.io workspace member whose version crates.io does not
# already hold.
#
# `cargo publish --workspace` (stable since 1.90) orders the crates by their
# dependencies and waits for the index between them, but aborts on the first
# version crates.io already has (rust-lang/cargo#13397). A release usually bumps
# only some crates, so this asks crates.io which versions exist and --excludes
# them, leaving cargo the crates that need uploading. A re-run after a partial
# failure is therefore safe: whatever already landed is excluded next time.
#
# The decision logic is split into `crate_published` and `publish_plan` so each
# reads on its own. The "Publish script" CI job runs shellcheck over this file so
# a shell mistake fails a pull request rather than a release.
set -euo pipefail

# Whether crates.io already holds an exact `name@version`.
#
#   returns 0  the version is published
#   returns 1  the version is genuinely absent, so it is ours to publish
#   exits 1    any other `cargo info` failure (network, auth, registry down)
#
# `cargo info` exits non-zero both when a version is missing and when it cannot
# reach the registry at all, so the exit code alone cannot tell them apart. Only
# the "could not find" message means a successful lookup that found nothing;
# every other failure aborts, because guessing "absent" would either publish
# over a real problem or abort the release on the first already-published crate.
crate_published() {
  local spec=$1 out code
  if out=$(cargo info --quiet --registry crates-io "$spec" 2>&1); then
    return 0
  fi
  code=$?
  if grep -qF "could not find \`$spec\` in registry" <<<"$out"; then
    return 1
  fi
  printf 'cargo info %s failed (exit %s):\n%s\n' "$spec" "$code" "$out" >&2
  exit 1
}

# Reads `<name> <version>` lines and prints one `publish <name>` or `skip <name>`
# line per crate, deciding by whether crates.io already holds the version.
publish_plan() {
  local name version
  while read -r name version; do
    [[ -z $name ]] && continue
    if crate_published "$name@$version"; then
      printf 'skip %s\n' "$name"
    else
      printf 'publish %s\n' "$name"
    fi
  done
}

main() {
  mkdir -p target
  cargo metadata --no-deps --format-version 1 --locked >target/metadata.json

  # Capturing the plan (rather than piping straight into the loop) keeps the
  # decision in this shell: a `crate_published` abort fails the assignment and,
  # under `set -e`, the whole run — instead of dying only in a pipe subshell.
  local plan
  plan=$(cargo xtask publishable | publish_plan)

  # dpp-python ships to PyPI, so it is absent from `publishable`, but it is a
  # workspace member `--workspace` would still try to publish. Always exclude it.
  local excluded=(dpp-python)
  local publishing=()
  local action name
  while read -r action name; do
    case $action in
      skip) excluded+=("$name") ;;
      publish) publishing+=("$name") ;;
    esac
  done <<<"$plan"

  if ((${#publishing[@]} == 0)); then
    echo "nothing to publish: every crate is already at its published version"
    return 0
  fi

  local flags=() crate
  for crate in "${excluded[@]}"; do
    flags+=(--exclude "$crate")
  done

  echo "publishing: ${publishing[*]}"
  echo "excluding:  ${excluded[*]}"
  # --no-verify: the package job already built and verified this commit.
  cargo publish --workspace --locked --no-verify "${flags[@]}"
}

# Run when executed; stay quiet when sourced by the test.
if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
  main "$@"
fi
