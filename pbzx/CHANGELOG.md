# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-02-03

### Added

- PBZX archive reading with XZ stream decompression
- PBZX archive writing (`PbzxWriter`)
- CPIO parsing for odc (`070707`), newc (`070701`), and crc (`070702`) formats
- CPIO building for odc and newc formats (`CpioBuilder`)
- Seek-based header parsing for fast file listing
- Streaming extraction to filesystem
- Feature flags: `extract`, `list`, `pack`
