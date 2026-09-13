# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Initial release: decode Apple `decmpfs` transparent filesystem compression
- `Header::parse` reads the `com.apple.decmpfs` attribute header;
  `Header::storage` reports whether the payload is inline, in the resource
  fork, dataless or of an unrecognised type, so callers fetch a resource fork
  only when one is needed
- `decompress` handles inline types 1, 3, 7, 9 and 11 and resource-fork types
  4, 8, 10 and 12, across zlib, LZVN, LZFSE and stored blocks
- LZBITMAP (types 13 and 14) and dataless files report distinct errors rather
  than being decoded wrongly
- `XattrKind` and `classify_xattr` tell a compression attribute apart from a
  user one. `com.apple.ResourceFork` is only machinery on a compressed file —
  on an uncompressed one it is user data, which is why a name alone cannot
  decide
- `tools/mint-fixtures.py` generates fixtures from a real macOS filesystem, and
  ignored tests in `tests/real_fixtures.rs` decode them
