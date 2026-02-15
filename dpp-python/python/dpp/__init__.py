"""
dpp - Python bindings for parsing Apple DMG disk images, HFS+/APFS filesystems,
PKG installers, and PBZX/CPIO payloads.

Quick start::

    import dpp

    # Open a DMG and explore its filesystem
    with dpp.open("installer.dmg") as dmg:
        for p in dmg.partitions:
            print(p.name, p.partition_type, p.size)

        with dmg.filesystem() as fs:
            for entry in fs.list_directory("/"):
                print(entry.name, entry.kind, entry.size)

            data = fs.read_file("/some/file.txt")
"""

from dpp._dpp import (
    # Top-level functions
    open,
    find_packages,
    extract_pkg_payload,
    # Pipeline classes
    DmgPipeline,
    FilesystemHandle,
    # DMG classes
    DmgArchive,
    DmgBuilder,
    # PKG/XAR classes
    PkgReader,
    XarArchive,
    # Payload classes
    Archive,
    CpioBuilder,
    PbzxWriter,
    # Filesystem classes
    HfsVolume,
    ApfsVolume,
    # Data types
    PartitionInfo,
    DirEntry,
    FileStat,
    VolumeInfo,
    WalkEntry,
    FileEntry,
    CompressionInfo,
    DmgStats,
    XarFile,
    ChunkInfo,
    ArchiveStats,
    # Exceptions
    DppError,
    IoError,
    InvalidFormatError,
    FileNotFoundError,
    DecompressionError,
    UnsupportedError,
)

__all__ = [
    # Functions
    "open",
    "find_packages",
    "extract_pkg_payload",
    # Pipeline
    "DmgPipeline",
    "FilesystemHandle",
    # DMG
    "DmgArchive",
    "DmgBuilder",
    # PKG/XAR
    "PkgReader",
    "XarArchive",
    # Payload
    "Archive",
    "CpioBuilder",
    "PbzxWriter",
    # Filesystem
    "HfsVolume",
    "ApfsVolume",
    # Data types
    "PartitionInfo",
    "DirEntry",
    "FileStat",
    "VolumeInfo",
    "WalkEntry",
    "FileEntry",
    "CompressionInfo",
    "DmgStats",
    "XarFile",
    "ChunkInfo",
    "ArchiveStats",
    # Exceptions
    "DppError",
    "IoError",
    "InvalidFormatError",
    "FileNotFoundError",
    "DecompressionError",
    "UnsupportedError",
]
