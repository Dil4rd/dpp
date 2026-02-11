pub mod error;
pub mod pipeline;

pub use error::{DppError, Result};
pub use pipeline::{DmgPipeline, ExtractMode, HfsHandle};

// Re-export underlying crates
pub use hfsplus;
pub use xara;
pub use udif;
pub use pbzx;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mode_default() {
        assert!(matches!(ExtractMode::default(), ExtractMode::TempFile));
    }

    #[test]
    fn test_error_display() {
        let err = DppError::NoHfsPartition;
        assert_eq!(err.to_string(), "no HFS+ partition found in DMG");

        let err = DppError::FileNotFound("test.pkg".to_string());
        assert_eq!(err.to_string(), "file not found: test.pkg");
    }
}
