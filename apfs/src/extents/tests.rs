//! Unit tests for the parent module, split out of `extents.rs` for size.

use super::*;
use crate::catalog::{FileExtentRecord, FileExtentVal};

#[test]
fn test_extent_block_number_beyond_the_address_space_is_rejected() {
    // phys_block_num comes straight off disk. Multiplied by the block size
    // it used to wrap, sending the read to an unrelated offset that looks
    // like recovered data.
    let extents = vec![FileExtentRecord {
        logical_addr: 0,
        value: FileExtentVal {
            flags_and_length: 4096,
            phys_block_num: u64::MAX,
            crypto_id: 0,
        },
    }];

    let mut reader = std::io::Cursor::new(vec![0u8; 8192]);
    let mut out = Vec::new();
    let err = read_file_data(&mut reader, 4096, &extents, 4096, &mut out).unwrap_err();

    assert!(
        matches!(err, ApfsError::CorruptedData(_)),
        "expected CorruptedData, got {err:?}"
    );
}

/// A 12 KiB file: 4 KiB of data, a 4 KiB hole, then 4 KiB of data. The
/// second extent's records says it lives at 8192; summing the lengths
/// before it would place it at 4096.
fn sparse_fork() -> (std::io::Cursor<Vec<u8>>, Vec<FileExtentRecord>, u64) {
    let mut disk = vec![0u8; 4096 * 3];
    disk[4096..8192].fill(b'A');
    disk[8192..12288].fill(b'B');

    let extents = vec![
        FileExtentRecord {
            logical_addr: 0,
            value: FileExtentVal {
                flags_and_length: 4096,
                phys_block_num: 1,
                crypto_id: 0,
            },
        },
        FileExtentRecord {
            logical_addr: 8192,
            value: FileExtentVal {
                flags_and_length: 4096,
                phys_block_num: 2,
                crypto_id: 0,
            },
        },
    ];
    (std::io::Cursor::new(disk), extents, 12288)
}

fn expected_sparse_contents() -> Vec<u8> {
    let mut expected = vec![b'A'; 4096];
    expected.extend(std::iter::repeat_n(0u8, 4096));
    expected.extend(std::iter::repeat_n(b'B', 4096));
    expected
}

#[test]
fn test_read_file_data_places_extents_at_their_logical_address() {
    let (mut reader, extents, logical_size) = sparse_fork();
    let mut out = Vec::new();
    let written = read_file_data(&mut reader, 4096, &extents, logical_size, &mut out).unwrap();

    assert_eq!(written, logical_size);
    assert_eq!(out, expected_sparse_contents());
}

#[test]
fn test_read_file_data_is_independent_of_record_order() {
    let (mut reader, mut extents, logical_size) = sparse_fork();
    extents.reverse();
    let mut out = Vec::new();
    read_file_data(&mut reader, 4096, &extents, logical_size, &mut out).unwrap();

    assert_eq!(out, expected_sparse_contents());
}

#[test]
fn test_fork_reader_reads_holes_as_zeros() {
    let (mut cursor, extents, logical_size) = sparse_fork();
    let mut fork = ApfsForkReader::new(&mut cursor, 4096, extents, logical_size);

    let mut out = Vec::new();
    fork.read_to_end(&mut out).unwrap();
    assert_eq!(out, expected_sparse_contents());
}

#[test]
fn test_fork_reader_seeks_past_a_hole() {
    // Previously this returned UnexpectedEof: the map placed the second
    // extent at 4096, so nothing covered offset 9000.
    let (mut cursor, extents, logical_size) = sparse_fork();
    let mut fork = ApfsForkReader::new(&mut cursor, 4096, extents, logical_size);

    fork.seek(SeekFrom::Start(9000)).unwrap();
    let mut out = vec![0u8; 8];
    fork.read_exact(&mut out).unwrap();
    assert_eq!(out, [b'B'; 8]);

    // And a read landing inside the hole yields zeros.
    fork.seek(SeekFrom::Start(5000)).unwrap();
    let mut out = vec![0xFFu8; 8];
    fork.read_exact(&mut out).unwrap();
    assert_eq!(out, [0u8; 8]);
}
