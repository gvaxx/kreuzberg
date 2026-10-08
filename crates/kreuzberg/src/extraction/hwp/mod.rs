//! Vendored HWP text extraction from hwpers v0.5.0 (MIT OR Apache-2.0)
//!
//! Supports HWP 5.0 Compound File Binary (CFB) documents.  Only text
//! extraction is implemented; write, render, crypto, HWPX, and preview paths
//! from the original crate are omitted.
//!
//! # Entry point
//!
//! ```ignore
//! let text = extract_hwp_text(bytes, DEFAULT_MAX_DECOMPRESSED_SIZE)?;
//! ```

pub mod error;
pub mod model;
pub mod parser;
pub mod reader;

use error::{HwpError, Result};
use model::HwpDocument;
use parser::{FileHeader, parse_body_text};
use reader::{CfbReader, decompress_stream};

/// Default cap on the total decompressed size of all body sections (500 MB),
/// matching `SecurityLimits::max_archive_size`.
pub const DEFAULT_MAX_DECOMPRESSED_SIZE: usize = 500 * 1024 * 1024;

/// Extract all plain text from an HWP 5.0 document given its raw bytes.
///
/// # Errors
///
/// Returns `HwpError` if the bytes do not form a valid HWP 5.0 compound file,
/// if the document is password-encrypted, if a critical parsing step fails,
/// or if the body sections together exceed `max_decompressed_size` bytes.
pub fn extract_hwp_text(bytes: &[u8], max_decompressed_size: usize) -> Result<String> {
    let mut cfb = CfbReader::from_bytes(bytes)?;

    // Parse the 256-byte file header
    let header_data = cfb.read_stream("FileHeader")?;
    let header = FileHeader::parse(header_data)?;

    if header.is_encrypted() {
        return Err(HwpError::UnsupportedVersion(
            "Password-encrypted HWP documents are not supported".to_string(),
        ));
    }

    // Distribution documents store body text under ViewText/SectionN
    let stream_prefix = if header.is_distribute() {
        "ViewText/Section"
    } else {
        "BodyText/Section"
    };

    let mut doc = HwpDocument::default();
    let mut section_idx = 0u32;
    let mut remaining_budget = max_decompressed_size;

    loop {
        let section_name = format!("{stream_prefix}{section_idx}");
        if !cfb.stream_exists(&section_name) {
            break;
        }

        let section_data = cfb.read_stream(&section_name)?;
        let section_data = if header.is_compressed() {
            decompress_stream(&section_data, remaining_budget)?
        } else {
            section_data
        };
        remaining_budget = remaining_budget.checked_sub(section_data.len()).ok_or_else(|| {
            HwpError::CompressionError(format!("HWP body sections exceed {max_decompressed_size} byte limit"))
        })?;

        let sections = parse_body_text(section_data)?;
        doc.sections.extend(sections);

        section_idx += 1;
    }

    if doc.sections.is_empty() {
        return Err(HwpError::InvalidFormat(
            "No BodyText sections found in HWP document".to_string(),
        ));
    }

    Ok(doc.extract_text())
}
