# XAR Format Specification

This document describes the XAR (eXtensible ARchive) format as implemented by the xara library.

## Overview

XAR is an archive format designed by Apple, used as the container format for macOS `.pkg` installer packages. It consists of a header, a compressed XML table of contents, and a data heap.

```
+----------------------------+
| Header (28 bytes)          |  Magic, sizes, checksum algo
+----------------------------+
| TOC (zlib compressed)      |  XML file listing
+----------------------------+
| Heap                       |  Compressed/raw file data
+----------------------------+
```

## Header

The XAR header is **28 bytes** at the start of the file.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 4 | u32 BE | Magic: `xar!` (0x78617221) |
| 4 | 2 | u16 BE | Header size (usually 28) |
| 6 | 2 | u16 BE | Version (1) |
| 8 | 8 | u64 BE | TOC compressed length |
| 16 | 8 | u64 BE | TOC uncompressed length |
| 24 | 4 | u32 BE | Checksum algorithm |

### Checksum Algorithms

| Value | Algorithm |
|-------|-----------|
| 0 | None |
| 1 | SHA-1 |
| 2 | MD5 |
| 3 | SHA-256 |

## Table of Contents (TOC)

The TOC is a zlib-compressed XML document. It contains a `<toc>` element with nested `<file>` elements:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<xar>
  <toc>
    <file id="1">
      <name>example.txt</name>
      <type>file</type>
      <data>
        <offset>0</offset>
        <length>1024</length>
        <size>4096</size>
        <encoding style="application/x-gzip"/>
        <extracted-checksum style="sha1">abc123...</extracted-checksum>
        <archived-checksum style="sha1">def456...</archived-checksum>
      </data>
    </file>
    <file id="2">
      <name>subdir</name>
      <type>directory</type>
      <file id="3">
        <name>nested.txt</name>
        <type>file</type>
        <data>...</data>
      </file>
    </file>
  </toc>
</xar>
```

### File Types

| Value | Description |
|-------|-------------|
| `file` | Regular file |
| `directory` | Directory |
| `symlink` | Symbolic link |

Symbolic-link targets are stored as exact character data in a direct `<link>`
child of the file entry:

```xml
<file id="4">
  <name>current</name>
  <type>symlink</type>
  <link type="file">releases/2.0</link>
</file>
```

### Base64-Encoded Names

A `<name>` element whose UTF-8 value is not representable in ISO-8859-1
carries `enctype="base64"` and holds the base64 encoding of the real name in
place of the literal text. macOS `xar` does this for names outside the
ISO-8859-1 repertoire, such as Japanese characters:

```xml
<file id="5">
  <name enctype="base64">44GT44KT44Gr44Gh44GvLnR4dA==</name>
  <type>file</type>
</file>
```

decodes to the name `こんにちは.txt`. Base64 line wrapping whitespace is
ignored. Any other `enctype` value, or base64 or UTF-8 that fails to decode, is
a parse error.

### Data Encoding Styles

| Style | Description |
|-------|-------------|
| `application/x-gzip` | Gzip/zlib compressed |
| `application/octet-stream` | Uncompressed |
| `application/x-bzip2` | Bzip2 compressed |

## Heap

The heap starts immediately after the compressed TOC (`header_size + toc_compressed_length`). File data is stored at offsets relative to the heap start.

## macOS PKG Structure

macOS `.pkg` installers are XAR archives with a specific internal structure:

### Product Package

```
Distribution          XML describing the package
com.example.pkg/      Component package directory
  PackageInfo          XML metadata
  Payload              PBZX-compressed CPIO archive
  Bom                  Bill of Materials
  Scripts              PBZX-compressed CPIO of install scripts
```

### Component Package

```
PackageInfo           XML metadata
Payload               PBZX-compressed CPIO archive
Bom                   Bill of Materials
Scripts               PBZX-compressed CPIO of install scripts
```

Product packages have a `Distribution` XML file at the root and contain one or more component sub-packages as directories. Component packages have `Payload` and `PackageInfo` directly at the root level.
