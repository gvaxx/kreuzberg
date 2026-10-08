"""Extract every HWP fixture in a directory with the installed wheel."""
import sys
from pathlib import Path
from kreuzberg import ExtractionConfig, extract_file_sync

paths = sorted(Path(sys.argv[1]).glob('*.hwp'))
assert paths, 'No HWP fixtures found'
for path in paths:
    result = extract_file_sync(str(path), config=ExtractionConfig(use_cache=False))
    assert result.content.strip(), f'{path.name}: empty content'
    print('PASS:', path.name, len(result.content), 'chars')
