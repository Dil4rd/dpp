//! Unit tests for the parent module, split out of `reader.rs` for size.

use super::*;
use std::io::Cursor;

fn create_minimal_pbzx() -> Vec<u8> {
    let mut data = Vec::new();
    // Magic
    data.extend_from_slice(&PBZX_MAGIC);
    // Chunk size (8 bytes, big-endian): 16 MiB, as Apple's tooling writes.
    data.extend_from_slice(&0x0100_0000u64.to_be_bytes());
    data
}

#[test]
fn test_zero_chunk_header_at_eof_ends_the_stream() {
    let mut data = create_minimal_pbzx();
    data.extend_from_slice(&[0u8; 16]);

    let mut reader = PbzxReader::new(Cursor::new(data)).unwrap();
    let mut out = Vec::new();

    assert_eq!(reader.decompress_to(&mut out).unwrap(), 0);
    assert!(out.is_empty());
}

#[test]
fn test_zero_chunk_header_followed_by_data_is_rejected() {
    // Apple's payloads end at EOF rather than with a marker, so a zero
    // header with data behind it is corruption. Accepting it would report
    // a truncated archive as a complete one.
    let mut data = create_minimal_pbzx();
    data.extend_from_slice(&[0u8; 16]);
    data.extend_from_slice(&[0u8; 8]);
    data.extend_from_slice(b"more chunks would follow");

    let mut reader = PbzxReader::new(Cursor::new(data)).unwrap();
    let mut out = Vec::new();

    let err = reader.decompress_to(&mut out).unwrap_err();

    assert!(
        matches!(err, PbzxError::InvalidChunk { .. }),
        "expected InvalidChunk, got {err:?}"
    );
}

#[test]
fn test_header_parsing() {
    let data = create_minimal_pbzx();
    let cursor = Cursor::new(data);
    let reader = PbzxReader::new(cursor).unwrap();

    assert!(reader.header().is_valid());
    assert_eq!(reader.chunk_size(), 0x0100_0000);
}

#[test]
fn test_invalid_magic() {
    let data = vec![0x00, 0x00, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0];
    let cursor = Cursor::new(data);
    let result = PbzxReader::new(cursor);

    assert!(matches!(result, Err(PbzxError::InvalidMagic(_))));
}
