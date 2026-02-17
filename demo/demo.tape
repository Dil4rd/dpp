# demo.tape — VHS recording for dpp-tool
# Usage: vhs demo/demo.tape

Output demo/demo.gif
Output demo/demo.mp4

Set Shell "bash"
Set FontSize 14
Set FontFamily "JetBrains Mono"
Set Width 1000
Set Height 750
Set Theme "Dracula"
Set TypingSpeed 50ms
Set Padding 20
Set WindowBar Colorful

# Hidden setup: cd to tests/, bash function wrapper, custom prompt
Hide
Type "cd tests"
Enter
Type 'dpp-tool() { ../target/release/dpp-tool "$@"; }'
Enter
Type 'export PS1="$ "'
Enter
Type "clear"
Enter
Sleep 500ms
Show

# Header
Type@0ms '#  dpp-tool demo — Apple DMG Pipeline Explorer'
Enter
Type@0ms '# ─────────────────────────────────────────────'
Enter

# 1. List partitions
Type@60ms "# list partitions inside DMG"
Enter
Type "dpp-tool dmg ls kdk.dmg"
Sleep 500ms
Enter
Wait /\$/
Sleep 5s

# 2. Browse filesystem
Type@60ms "# browse the HFS+ filesystem inside the DMG"
Enter
Type "dpp-tool fs ls kdk.dmg /"
Sleep 500ms
Enter
Wait /\$/
Sleep 5s

# 3. Inspect PKG
Type@60ms "# inspect KernelDebugKit.pkg installer"
Enter
Type "dpp-tool pkg info kdk.dmg /KernelDebugKit.pkg"
Sleep 500ms
Enter
Wait /\$/
Sleep 5s

# 4. Drill into payload
Type@60ms "# drill into KDK_SDK.pkg to see what it installs"
Enter
Type "dpp-tool payload tree kdk.dmg /KernelDebugKit.pkg KDK_SDK.pkg --depth 1"
Sleep 1s
Enter
Wait /\$/
Sleep 10s
