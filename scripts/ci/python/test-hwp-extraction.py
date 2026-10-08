"""Extract every HWP fixture in a directory with the installed wheel."""
import sys
from pathlib import Path
from kreuzberg import ExtractionConfig, extract_file_sync

paths = sorted(Path(sys.argv[1]).glob('*.hwp'))
assert paths, 'No HWP fixtures found'
extracted = 0
for path in paths:
    # styled_document.hwp has no text the vendored parser can read, even before 4.9.6.
    result = extract_file_sync(str(path), config=ExtractionConfig(use_cache=False))
    extracted += bool(result.content.strip())
    print('PASS:', path.name, len(result.content), 'chars')
assert extracted, 'No text extracted from any HWP fixture'
