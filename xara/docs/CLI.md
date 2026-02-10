# CLI Tool

The xara crate does not include a standalone CLI tool. For XAR/PKG operations, use the `dpp-tool` which integrates xara into the full pipeline.

## Via dpp-tool

```bash
# Build the pipeline tool
cargo build --example dpp-tool --release -p dpp

# Inspect a PKG inside a DMG
cargo run --example dpp-tool -- pkg-info image.dmg /path/to/installer.pkg

# Extract a component payload
cargo run --example dpp-tool -- extract image.dmg /path.pkg com.apple.pkg.KDK ./output/
```

## Programmatic Usage

```rust
use xara::{XarArchive, PkgReader};
use std::fs::File;
use std::io::BufReader;

// Open XAR directly
let file = File::open("archive.xar")?;
let mut xar = XarArchive::open(BufReader::new(file))?;

// Or open as PKG
let file = File::open("installer.pkg")?;
let mut pkg = PkgReader::open(BufReader::new(file))?;

// List components
for comp in pkg.components() {
    println!("Component: {}", comp);
}

// Extract payload
let payload = pkg.payload("com.apple.pkg.KDK")?;
```
