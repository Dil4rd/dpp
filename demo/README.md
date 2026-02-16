# Demo Recording

## Prerequisites

- [asciinema](https://asciinema.org/) for terminal recording
- [agg](https://github.com/asciinema/agg) for GIF conversion
- `tests/kdk.dmg` test fixture (976 MB Kernel Debug Kit)
- `dpp-tool` built with: `cargo build --release --features parallel`

## Record

```bash
TERM=xterm-256color asciinema rec \
    --cols 100 --rows 35 \
    --idle-time-limit 3 \
    --title "dpp-tool: Apple DMG Pipeline Explorer" \
    --command ./demo/record.sh \
    --overwrite \
    demo/demo.cast
```

## Preview

```bash
asciinema play demo/demo.cast
```

## Convert to GIF

Pick a monospace font installed on your system (`fc-list :spacing=mono family`):

```bash
agg --font-family "Ubuntu Mono" \
    --font-size 14 \
    --theme monokai \
    demo/demo.cast demo/demo.gif
```

Note: `agg` does not support comma-separated fallback lists. Pass a single
font family name that exists on your system (e.g. `JetBrains Mono`, `Menlo`,
`Ubuntu Mono`).

If the GIF exceeds 5 MB, try `agg --speed 1.5` or reduce `--rows`.
