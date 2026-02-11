/// Fletcher-64 checksum used by APFS.
///
/// Every on-disk object has a 64-bit checksum at offset 0, computed over
/// bytes 8..block_size using a modular Fletcher-64 variant.

/// Compute APFS Fletcher-64 checksum over a byte slice.
///
/// The input should be the object data starting at offset 8 (skipping the
/// checksum field itself). Data length must be a multiple of 4.
pub fn fletcher64(data: &[u8]) -> u64 {
    // APFS uses a variant of Fletcher-64 that operates on 32-bit words.
    // The modulus is 2^32 - 1 (0xFFFFFFFF).
    let mod_val: u64 = 0xFFFFFFFF;

    let mut sum1: u64 = 0;
    let mut sum2: u64 = 0;

    // Process 4 bytes at a time (little-endian u32)
    for chunk in data.chunks_exact(4) {
        let word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as u64;
        sum1 = (sum1 + word) % mod_val;
        sum2 = (sum2 + sum1) % mod_val;
    }

    let check1 = mod_val - ((sum1 + sum2) % mod_val);
    let check2 = mod_val - ((sum1 + check1) % mod_val);

    (check2 << 32) | check1
}

/// Verify the Fletcher-64 checksum of an APFS on-disk object block.
///
/// The block must be at least 8 bytes (checksum at offset 0..8, data at 8..).
/// Returns true if the stored checksum matches the computed checksum.
pub fn verify_object(block: &[u8]) -> bool {
    if block.len() < 8 {
        return false;
    }

    let stored = u64::from_le_bytes([
        block[0], block[1], block[2], block[3],
        block[4], block[5], block[6], block[7],
    ]);

    let computed = fletcher64(&block[8..]);
    stored == computed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fletcher64_known() {
        let path = std::path::Path::new("../tests/appfs.raw");
        if !path.exists() {
            eprintln!("Skipping test - appfs.raw not found");
            return;
        }

        // Read block 0 (the container superblock)
        let mut file = std::fs::File::open(path).unwrap();
        use std::io::Read;
        let mut block = vec![0u8; 4096];
        file.read_exact(&mut block).unwrap();

        assert!(verify_object(&block), "Block 0 checksum should be valid");

        // Also verify that the computed checksum matches the stored one
        let stored = u64::from_le_bytes([
            block[0], block[1], block[2], block[3],
            block[4], block[5], block[6], block[7],
        ]);
        let computed = fletcher64(&block[8..]);
        assert_eq!(stored, computed,
            "Stored checksum 0x{:016X} should match computed 0x{:016X}", stored, computed);
    }
}
