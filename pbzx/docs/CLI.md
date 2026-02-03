# CLI Tool

The `pbzx-tool` example provides a command-line interface for working with PBZX archives.

## Building

```bash
cargo build --example pbzx-tool --release
```

## Commands

### info

Display archive metadata and statistics.

```bash
cargo run --example pbzx-tool -- info Payload
```

### list

List all files in the archive.

```bash
cargo run --example pbzx-tool -- list Payload
```

### extract

Extract all files to a directory.

```bash
cargo run --example pbzx-tool -- extract Payload output_dir
```

### cat

Extract a single file to stdout.

```bash
cargo run --example pbzx-tool -- cat Payload path/to/file
```

## Examples

List files in a macOS installer payload:

```bash
cargo run --example pbzx-tool --release -- list /path/to/package.pkg/Payload
```

Extract a specific binary:

```bash
cargo run --example pbzx-tool --release -- cat Payload usr/bin/some-tool > some-tool
chmod +x some-tool
```

Extract entire archive:

```bash
cargo run --example pbzx-tool --release -- extract Payload ./extracted/
```
