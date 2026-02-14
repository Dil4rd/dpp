//! HFS+ Unicode comparison utilities.
//!
//! HFSX (case-sensitive) uses binary comparison of UTF-16BE values.
//! HFS+ (case-insensitive) uses Apple's FastUnicodeCompare with a
//! case-folding table defined in Apple TN1150.

/// Compare two HFS+ Unicode names using binary comparison (HFSX / case-sensitive).
pub fn compare_binary(a: &[u16], b: &[u16]) -> std::cmp::Ordering {
    let len = a.len().min(b.len());
    for i in 0..len {
        match a[i].cmp(&b[i]) {
            std::cmp::Ordering::Equal => continue,
            ord => return ord,
        }
    }
    a.len().cmp(&b.len())
}

/// Case-folding table for HFS+ FastUnicodeCompare (from Apple TN1150).
/// Maps Unicode code points to their case-folded equivalents.
/// Only entries that differ from identity are listed.
static CASE_FOLD: &[(u16, u16)] = &[
    (0x0041, 0x0061), // A -> a
    (0x0042, 0x0062), // B -> b
    (0x0043, 0x0063), // C -> c
    (0x0044, 0x0064), // D -> d
    (0x0045, 0x0065), // E -> e
    (0x0046, 0x0066), // F -> f
    (0x0047, 0x0067), // G -> g
    (0x0048, 0x0068), // H -> h
    (0x0049, 0x0069), // I -> i
    (0x004A, 0x006A), // J -> j
    (0x004B, 0x006B), // K -> k
    (0x004C, 0x006C), // L -> l
    (0x004D, 0x006D), // M -> m
    (0x004E, 0x006E), // N -> n
    (0x004F, 0x006F), // O -> o
    (0x0050, 0x0070), // P -> p
    (0x0051, 0x0071), // Q -> q
    (0x0052, 0x0072), // R -> r
    (0x0053, 0x0073), // S -> s
    (0x0054, 0x0074), // T -> t
    (0x0055, 0x0075), // U -> u
    (0x0056, 0x0076), // V -> v
    (0x0057, 0x0077), // W -> w
    (0x0058, 0x0078), // X -> x
    (0x0059, 0x0079), // Y -> y
    (0x005A, 0x007A), // Z -> z
    (0x00C0, 0x00E0), // À -> à
    (0x00C1, 0x00E1), // Á -> á
    (0x00C2, 0x00E2), // Â -> â
    (0x00C3, 0x00E3), // Ã -> ã
    (0x00C4, 0x00E4), // Ä -> ä
    (0x00C5, 0x00E5), // Å -> å
    (0x00C6, 0x00E6), // Æ -> æ
    (0x00C7, 0x00E7), // Ç -> ç
    (0x00C8, 0x00E8), // È -> è
    (0x00C9, 0x00E9), // É -> é
    (0x00CA, 0x00EA), // Ê -> ê
    (0x00CB, 0x00EB), // Ë -> ë
    (0x00CC, 0x00EC), // Ì -> ì
    (0x00CD, 0x00ED), // Í -> í
    (0x00CE, 0x00EE), // Î -> î
    (0x00CF, 0x00EF), // Ï -> ï
    (0x00D0, 0x00F0), // Ð -> ð
    (0x00D1, 0x00F1), // Ñ -> ñ
    (0x00D2, 0x00F2), // Ò -> ò
    (0x00D3, 0x00F3), // Ó -> ó
    (0x00D4, 0x00F4), // Ô -> ô
    (0x00D5, 0x00F5), // Õ -> õ
    (0x00D6, 0x00F6), // Ö -> ö
    (0x00D8, 0x00F8), // Ø -> ø
    (0x00D9, 0x00F9), // Ù -> ù
    (0x00DA, 0x00FA), // Ú -> ú
    (0x00DB, 0x00FB), // Û -> û
    (0x00DC, 0x00FC), // Ü -> ü
    (0x00DD, 0x00FD), // Ý -> ý
    (0x00DE, 0x00FE), // Þ -> þ
    (0x0100, 0x0101), // Ā -> ā
    (0x0102, 0x0103), // Ă -> ă
    (0x0104, 0x0105), // Ą -> ą
    (0x0106, 0x0107), // Ć -> ć
    (0x0108, 0x0109), // Ĉ -> ĉ
    (0x010A, 0x010B), // Ċ -> ċ
    (0x010C, 0x010D), // Č -> č
    (0x010E, 0x010F), // Ď -> ď
    (0x0110, 0x0111), // Đ -> đ
    (0x0112, 0x0113), // Ē -> ē
    (0x0114, 0x0115), // Ĕ -> ĕ
    (0x0116, 0x0117), // Ė -> ė
    (0x0118, 0x0119), // Ę -> ę
    (0x011A, 0x011B), // Ě -> ě
    (0x011C, 0x011D), // Ĝ -> ĝ
    (0x011E, 0x011F), // Ğ -> ğ
    (0x0120, 0x0121), // Ġ -> ġ
    (0x0122, 0x0123), // Ģ -> ģ
    (0x0124, 0x0125), // Ĥ -> ĥ
    (0x0126, 0x0127), // Ħ -> ħ
    (0x0128, 0x0129), // Ĩ -> ĩ
    (0x012A, 0x012B), // Ī -> ī
    (0x012C, 0x012D), // Ĭ -> ĭ
    (0x012E, 0x012F), // Į -> į
    (0x0130, 0x0069), // İ -> i (Turkish I)
    (0x0132, 0x0133), // Ĳ -> ĳ
    (0x0134, 0x0135), // Ĵ -> ĵ
    (0x0136, 0x0137), // Ķ -> ķ
    (0x0139, 0x013A), // Ĺ -> ĺ
    (0x013B, 0x013C), // Ļ -> ļ
    (0x013D, 0x013E), // Ľ -> ľ
    (0x013F, 0x0140), // Ŀ -> ŀ
    (0x0141, 0x0142), // Ł -> ł
    (0x0143, 0x0144), // Ń -> ń
    (0x0145, 0x0146), // Ņ -> ņ
    (0x0147, 0x0148), // Ň -> ň
    (0x014A, 0x014B), // Ŋ -> ŋ
    (0x014C, 0x014D), // Ō -> ō
    (0x014E, 0x014F), // Ŏ -> ŏ
    (0x0150, 0x0151), // Ő -> ő
    (0x0152, 0x0153), // Œ -> œ
    (0x0154, 0x0155), // Ŕ -> ŕ
    (0x0156, 0x0157), // Ŗ -> ŗ
    (0x0158, 0x0159), // Ř -> ř
    (0x015A, 0x015B), // Ś -> ś
    (0x015C, 0x015D), // Ŝ -> ŝ
    (0x015E, 0x015F), // Ş -> ş
    (0x0160, 0x0161), // Š -> š
    (0x0162, 0x0163), // Ţ -> ţ
    (0x0164, 0x0165), // Ť -> ť
    (0x0166, 0x0167), // Ŧ -> ŧ
    (0x0168, 0x0169), // Ũ -> ũ
    (0x016A, 0x016B), // Ū -> ū
    (0x016C, 0x016D), // Ŭ -> ŭ
    (0x016E, 0x016F), // Ů -> ů
    (0x0170, 0x0171), // Ű -> ű
    (0x0172, 0x0173), // Ų -> ų
    (0x0174, 0x0175), // Ŵ -> ŵ
    (0x0176, 0x0177), // Ŷ -> ŷ
    (0x0178, 0x00FF), // Ÿ -> ÿ
    (0x0179, 0x017A), // Ź -> ź
    (0x017B, 0x017C), // Ż -> ż
    (0x017D, 0x017E), // Ž -> ž
];

/// Case-fold a single code point for HFS+ comparison
fn case_fold(c: u16) -> u16 {
    // Binary search through the case-fold table
    match CASE_FOLD.binary_search_by_key(&c, |&(from, _)| from) {
        Ok(idx) => CASE_FOLD[idx].1,
        Err(_) => c,
    }
}

/// Compare two HFS+ Unicode names using FastUnicodeCompare (case-insensitive).
pub fn compare_case_insensitive(a: &[u16], b: &[u16]) -> std::cmp::Ordering {
    let len = a.len().min(b.len());
    for i in 0..len {
        let fa = case_fold(a[i]);
        let fb = case_fold(b[i]);
        match fa.cmp(&fb) {
            std::cmp::Ordering::Equal => continue,
            ord => return ord,
        }
    }
    a.len().cmp(&b.len())
}

/// Convert a UTF-16BE byte slice to a Vec<u16> of code points
pub fn utf16be_to_u16(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect()
}

/// Convert a Vec<u16> of UTF-16 code points to a Rust String
pub fn utf16_to_string(code_points: &[u16]) -> String {
    String::from_utf16_lossy(code_points)
}

/// Encode a Rust string to HFS+ UTF-16BE name
pub fn string_to_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_compare() {
        let a = string_to_utf16("abc");
        let b = string_to_utf16("abd");
        assert_eq!(compare_binary(&a, &b), std::cmp::Ordering::Less);

        let a = string_to_utf16("abc");
        let b = string_to_utf16("abc");
        assert_eq!(compare_binary(&a, &b), std::cmp::Ordering::Equal);

        let a = string_to_utf16("abc");
        let b = string_to_utf16("ab");
        assert_eq!(compare_binary(&a, &b), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_case_insensitive_compare() {
        let a = string_to_utf16("ABC");
        let b = string_to_utf16("abc");
        assert_eq!(compare_case_insensitive(&a, &b), std::cmp::Ordering::Equal);

        let a = string_to_utf16("Hello");
        let b = string_to_utf16("hello");
        assert_eq!(compare_case_insensitive(&a, &b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_utf16_roundtrip() {
        let original = "Hello, World!";
        let utf16 = string_to_utf16(original);
        let back = utf16_to_string(&utf16);
        assert_eq!(original, back);
    }
}
