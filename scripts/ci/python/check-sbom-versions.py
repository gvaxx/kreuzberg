"""Fail if the wheel's embedded CycloneDX SBOM lists a crate below its security-fixed version."""
import json
import sys
import zipfile

# Minimum versions that fix the advisories from the 4.9.5 security review.
MIN_VERSIONS = {
    'lopdf': (0, 42, 0),  # RUSTSEC-2026-0187
    'quick-xml': (0, 41, 0),  # RUSTSEC-2026-0194, RUSTSEC-2026-0195
    'pyo3': (0, 29, 0),  # GHSA-36hh-v3qg-5jq4, GHSA-chgr-c6px-7xpp
    'openssl': (0, 10, 80),  # GHSA-phqj-4mhp-q6mq, GHSA-xp3w-r5p5-63rr, GHSA-xv59-967r-8726
    'quinn-proto': (0, 11, 15),  # GHSA-4w2j-m93h-cj5j
    'rmcp': (2, 1, 0),  # GHSA-33f5-2c5q-wgwj, GHSA-9g45-5xwm-f3wc, GHSA-9pj6-vhgr-3mwh
}


def parse_version(version):
    return tuple(int(part) for part in version.split('-')[0].split('+')[0].split('.')[:3])


failed = False
for wheel in sys.argv[1:]:
    with zipfile.ZipFile(wheel) as z:
        sboms = [n for n in z.namelist() if '.dist-info/sboms/' in n and n.endswith('.json')]
        assert sboms, f'{wheel}: no CycloneDX SBOM found'
        components = [c for name in sboms for c in json.loads(z.read(name)).get('components', [])]
    for component in components:
        minimum = MIN_VERSIONS.get(component.get('name'))
        if minimum is None:
            continue
        version = component['version']
        ok = parse_version(version) >= minimum
        failed |= not ok
        print(f"{'OK  ' if ok else 'FAIL'} {component['name']} {version} (>= {'.'.join(map(str, minimum))})")
sys.exit(1 if failed else 0)
