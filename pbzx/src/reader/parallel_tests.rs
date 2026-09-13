//! Unit tests for the parent module, split out of `reader.rs` for size.

use super::*;
use std::io::Cursor;

/// Create a multi-chunk PBZX archive for testing.
fn create_multi_chunk_pbzx(chunk_size: usize) -> (Vec<u8>, Vec<u8>) {
    use crate::writer::{CpioBuilder, PbzxWriter};

    let mut cpio_builder = CpioBuilder::new();
    for i in 0..10 {
        let content = format!(
            "File {} content with enough data to generate multiple chunks: {}",
            i,
            "abcdefghijklmnopqrstuvwxyz ".repeat(20)
        );
        cpio_builder.add_file(&format!("file_{}.txt", i), content.as_bytes(), 0o644);
    }
    let cpio_data = cpio_builder.finish();

    let mut pbzx_data = Vec::new();
    let mut writer = PbzxWriter::new(&mut pbzx_data)
        .chunk_size(chunk_size)
        .compression_level(1);
    writer.write_cpio(&cpio_data).unwrap();
    writer.finish().unwrap();

    (pbzx_data, cpio_data)
}

#[test]
fn test_parallel_matches_sequential() {
    let (pbzx_data, _) = create_multi_chunk_pbzx(256);

    // Sequential decompress
    let mut reader1 = PbzxReader::new(Cursor::new(&pbzx_data)).unwrap();
    let sequential = reader1.decompress().unwrap();

    // Parallel decompress
    let mut reader2 = PbzxReader::new(Cursor::new(&pbzx_data)).unwrap();
    let parallel = reader2.decompress_parallel().unwrap();

    assert_eq!(sequential, parallel);
}

#[test]
fn test_parallel_single_chunk() {
    let (pbzx_data, cpio_data) = create_multi_chunk_pbzx(1024 * 1024);

    let mut reader = PbzxReader::new(Cursor::new(&pbzx_data)).unwrap();
    let result = reader.decompress_parallel().unwrap();

    assert_eq!(result, cpio_data);
}

#[test]
fn test_parallel_empty_archive() {
    let mut data = Vec::new();
    data.extend_from_slice(&PBZX_MAGIC);
    data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]); // flags

    let mut reader = PbzxReader::new(Cursor::new(data)).unwrap();
    let result = reader.decompress_parallel().unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_parallel_to_writer() {
    let (pbzx_data, _) = create_multi_chunk_pbzx(256);

    // Sequential
    let mut reader1 = PbzxReader::new(Cursor::new(&pbzx_data)).unwrap();
    let sequential = reader1.decompress().unwrap();

    // Parallel via decompress_parallel_to
    let mut reader2 = PbzxReader::new(Cursor::new(&pbzx_data)).unwrap();
    let mut output = Vec::new();
    reader2.decompress_parallel_to(&mut output).unwrap();

    assert_eq!(sequential, output);
}
