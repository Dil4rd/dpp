#!/usr/bin/env bash
# Demo recording script for dpp-tool.
# Intended to be run via: asciinema rec --command ./demo/record.sh demo/demo.cast
#
# Simulates natural typing and runs representative dpp-tool commands
# against the Kernel Debug Kit DMG test fixture.

set -euo pipefail

DMG="tests/kdk.dmg"
DPP="./target/release/dpp-tool"

# Print characters one at a time with random delays for natural-looking typing.
type_cmd() {
    local cmd="$1"
    printf '\n$ '
    for (( i=0; i<${#cmd}; i++ )); do
        printf '%s' "${cmd:$i:1}"
        # Random delay between 20-70ms
        sleep "0.0$(( RANDOM % 3 + 4 ))$(( RANDOM % 10 ))"
    done
    printf '\n'
    sleep 0.3
}

# Print a dim comment line.
comment() {
    local display="# $1"
    shift
    type_cmd "$display"
    printf '$ '
    "$@"
    sleep 1
}

# Run a command with simulated typing, then pause for readability.
run() {
    local display="$1"
    shift
    type_cmd "$display"
    "$@"
    sleep 2
}

clear
printf '\033[1;36m  dpp-tool demo — Apple DMG Pipeline Explorer\033[0m\n'
printf '  ─────────────────────────────────────────────\n'
sleep 1.5

# 1. DMG container — what partitions are inside?
comment "list partitions inside DMG"
run "dpp-tool dmg ls kdk.dmg" \
    "$DPP" dmg ls "$DMG"

# 2. Filesystem — browse the HFS+ volume
comment "browse the HFS+ filesystem inside the DMG"
run "dpp-tool fs tree kdk.dmg / --depth 2" \
    "$DPP" fs tree "$DMG" / --depth 2

# 3. PKG — inspect the installer package
comment "inspect the PKG installer we found"
run "dpp-tool pkg info kdk.dmg /KernelDebugKit.pkg" \
    "$DPP" pkg info "$DMG" /KernelDebugKit.pkg

# 4. Payload — drill into the deepest layer
comment "drill into a payload to see what it installs"
run "dpp-tool payload tree kdk.dmg /KernelDebugKit.pkg KDK_SDK.pkg --depth 4" \
    "$DPP" payload tree "$DMG" /KernelDebugKit.pkg KDK_SDK.pkg --depth 4

# 5. Quick reference of all commands
comment "all available commands"
run "dpp-tool --help" \
    "$DPP" --help

sleep 2
