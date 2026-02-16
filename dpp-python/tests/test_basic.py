"""Basic tests for the dpp Python bindings.

These tests verify the module loads correctly and classes are accessible.
Tests that require DMG fixtures are skipped if no fixture is available.
"""

import os
import pytest

import dpp


# ── Module-level tests ──────────────────────────────────────────────────


def test_module_imports():
    """All public names should be importable."""
    assert hasattr(dpp, "open")
    assert hasattr(dpp, "find_packages")
    assert hasattr(dpp, "extract_pkg_payload")
    assert hasattr(dpp, "DmgPipeline")
    assert hasattr(dpp, "FilesystemHandle")
    assert hasattr(dpp, "DmgArchive")
    assert hasattr(dpp, "DmgBuilder")
    assert hasattr(dpp, "PkgReader")
    assert hasattr(dpp, "XarArchive")
    assert hasattr(dpp, "Archive")
    assert hasattr(dpp, "CpioBuilder")
    assert hasattr(dpp, "PbzxWriter")
    assert hasattr(dpp, "HfsVolume")
    assert hasattr(dpp, "ApfsVolume")


def test_data_types_importable():
    """Data types should be importable."""
    assert hasattr(dpp, "PartitionInfo")
    assert hasattr(dpp, "DirEntry")
    assert hasattr(dpp, "FileStat")
    assert hasattr(dpp, "VolumeInfo")
    assert hasattr(dpp, "WalkEntry")
    assert hasattr(dpp, "FileEntry")
    assert hasattr(dpp, "CompressionInfo")
    assert hasattr(dpp, "DmgStats")
    assert hasattr(dpp, "XarFile")
    assert hasattr(dpp, "ChunkInfo")
    assert hasattr(dpp, "ArchiveStats")


def test_exceptions_importable():
    """Exception types should be importable and form a hierarchy."""
    assert issubclass(dpp.IoError, dpp.DppError)
    assert issubclass(dpp.InvalidFormatError, dpp.DppError)
    assert issubclass(dpp.FileNotFoundError, dpp.DppError)
    assert issubclass(dpp.DecompressionError, dpp.DppError)
    assert issubclass(dpp.UnsupportedError, dpp.DppError)
    assert issubclass(dpp.DppError, Exception)


# ── Error handling tests ────────────────────────────────────────────────


def test_open_nonexistent_file():
    """Opening a nonexistent file should raise IoError."""
    with pytest.raises(dpp.IoError):
        dpp.open("/nonexistent/path/to/file.dmg")


def test_open_invalid_dmg(tmp_path):
    """Opening an invalid DMG should raise InvalidFormatError."""
    bad_dmg = tmp_path / "bad.dmg"
    bad_dmg.write_bytes(b"this is not a dmg file at all")
    with pytest.raises((dpp.InvalidFormatError, dpp.IoError)):
        dpp.open(str(bad_dmg))


def test_dmg_archive_nonexistent():
    """DmgArchive.open on nonexistent file should raise IoError."""
    with pytest.raises(dpp.IoError):
        dpp.DmgArchive.open("/nonexistent/path.dmg")


# ── CpioBuilder tests ──────────────────────────────────────────────────


def test_cpio_builder_basic():
    """CpioBuilder should create valid CPIO archives."""
    cpio = dpp.CpioBuilder()
    cpio.add_directory(".", mode=0o755)
    cpio.add_directory("./usr", mode=0o755)
    cpio.add_directory("./usr/bin", mode=0o755)
    cpio.add_file("./usr/bin/hello", b"#!/bin/sh\necho hello\n", mode=0o755)
    data = cpio.finish()

    assert isinstance(data, bytes)
    assert len(data) > 0
    # CPIO magic: "070701"
    assert data[:6] == b"070701"


def test_cpio_builder_symlink():
    """CpioBuilder should support symlinks."""
    cpio = dpp.CpioBuilder()
    cpio.add_directory(".", mode=0o755)
    cpio.add_file("./target", b"content", mode=0o644)
    cpio.add_symlink("./link", "./target", mode=0o777)
    data = cpio.finish()

    assert isinstance(data, bytes)
    assert len(data) > 0


def test_cpio_builder_repr():
    """CpioBuilder repr should indicate state."""
    cpio = dpp.CpioBuilder()
    assert "CpioBuilder" in repr(cpio)
    cpio.finish()
    assert "finished" in repr(cpio)


def test_cpio_builder_double_finish():
    """Finishing a CpioBuilder twice should raise."""
    cpio = dpp.CpioBuilder()
    cpio.finish()
    with pytest.raises(RuntimeError):
        cpio.finish()


# ── DmgBuilder tests ───────────────────────────────────────────────────


def test_dmg_builder_repr():
    """DmgBuilder repr should show compression and partition count."""
    builder = dpp.DmgBuilder()
    assert "DmgBuilder" in repr(builder)
    assert "zlib" in repr(builder)


def test_dmg_builder_compression_property():
    """DmgBuilder compression should be gettable/settable."""
    builder = dpp.DmgBuilder()
    assert builder.compression == "zlib"

    builder.compression = "bzip2"
    assert builder.compression == "bzip2"

    builder.compression = "raw"
    assert builder.compression == "raw"

    builder.compression = "lzfse"
    assert builder.compression == "lzfse"


def test_dmg_builder_invalid_compression():
    """DmgBuilder should reject invalid compression methods."""
    builder = dpp.DmgBuilder()
    with pytest.raises(ValueError):
        builder.compression = "invalid"


def test_dmg_builder_roundtrip(tmp_path):
    """Build a DMG and verify it can be opened."""
    dmg_path = str(tmp_path / "test.dmg")

    # Create a simple partition with some data
    partition_data = b"\x00" * 4096
    builder = dpp.DmgBuilder()
    builder.compression = "zlib"
    builder.add_partition("test partition", partition_data)
    builder.build(dmg_path)

    assert os.path.exists(dmg_path)

    # Verify we can open it
    with dpp.DmgArchive.open(dmg_path) as archive:
        parts = archive.partitions
        assert len(parts) >= 1

        stats = archive.stats
        assert stats.partition_count >= 1

        info = archive.compression_info
        assert isinstance(info, dpp.CompressionInfo)


# ── Context manager tests ──────────────────────────────────────────────


def test_pipeline_context_manager_repr():
    """DmgPipeline should show open/closed state."""
    # Can't test with real DMG, but we can test the error path
    with pytest.raises(dpp.IoError):
        with dpp.open("/nonexistent.dmg") as dmg:
            pass


def test_dmg_archive_context_manager(tmp_path):
    """DmgArchive context manager should close on exit."""
    dmg_path = str(tmp_path / "test.dmg")
    builder = dpp.DmgBuilder()
    builder.add_partition("test", b"\x00" * 512)
    builder.build(dmg_path)

    archive = dpp.DmgArchive.open(dmg_path)
    assert "open" in repr(archive)
    archive.__exit__(None, None, None)
    assert "closed" in repr(archive)


# ── DMG fixture-based tests (skipped if no fixture) ────────────────────

DMG_FIXTURE = os.environ.get("DPP_TEST_DMG")


@pytest.mark.skipif(DMG_FIXTURE is None, reason="DPP_TEST_DMG not set")
def test_pipeline_open_and_list():
    """Open a real DMG and list partitions."""
    with dpp.open(DMG_FIXTURE) as dmg:
        parts = dmg.partitions
        assert len(parts) > 0
        for p in parts:
            assert isinstance(p, dpp.PartitionInfo)
            assert isinstance(p.name, str)
            assert isinstance(p.size, int)


@pytest.mark.skipif(DMG_FIXTURE is None, reason="DPP_TEST_DMG not set")
def test_pipeline_filesystem():
    """Open a real DMG filesystem and list root."""
    with dpp.open(DMG_FIXTURE) as dmg:
        with dmg.filesystem() as fs:
            assert fs.fs_type in ("hfsplus", "apfs")
            entries = fs.list_directory("/")
            assert len(entries) > 0
            for e in entries:
                assert isinstance(e, dpp.DirEntry)
                assert isinstance(e.name, str)
                assert e.kind in ("file", "directory", "symlink")


@pytest.mark.skipif(DMG_FIXTURE is None, reason="DPP_TEST_DMG not set")
def test_pipeline_walk():
    """Walk a DMG filesystem."""
    with dpp.open(DMG_FIXTURE) as dmg:
        with dmg.filesystem() as fs:
            entries = fs.walk()
            assert len(entries) > 0
            for e in entries:
                assert isinstance(e, dpp.WalkEntry)
                assert isinstance(e.path, str)
