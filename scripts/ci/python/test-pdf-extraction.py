"""Check that the wheel can load bundled PDFium and extract a PDF with images."""
import sys

from kreuzberg import ExtractionConfig, ImageExtractionConfig, extract_file_sync

result = extract_file_sync(
    sys.argv[1],
    config=ExtractionConfig(images=ImageExtractionConfig(extract_images=True), use_cache=False),
)
assert result.content.strip(), "No text extracted from PDF"
assert result.images, "No images extracted from PDF"
