#!/usr/bin/env python3
"""Package an already built Linux executable and its user-level installer."""
import hashlib
from pathlib import Path
import tarfile
import tomllib

root = Path(__file__).resolve().parent.parent
binary = root / 'target/release/bing-wallpaper-rs'
if not binary.is_file():
    raise SystemExit('请先执行 cargo build --release --locked')
version = tomllib.loads((root / 'Cargo.toml').read_text())['package']['version']
name = f'bing-wallpaper-rs-{version}-linux-x86_64'
dist = root / 'dist'
dist.mkdir(exist_ok=True)
archive = dist / f'{name}.tar.gz'
with tarfile.open(archive, 'w:gz') as output:
    output.add(binary, arcname=f'{name}/bin/bing-wallpaper-rs')
    for path in ['README.md', 'CONTRIBUTING.md', 'CHANGELOG.md', 'LICENSE', 'scripts/install.py', 'scripts/uninstall.py', 'assets/bing-wallpaper-rs.svg', 'assets/screenshot.png']:
        output.add(root / path, arcname=f'{name}/{path}')
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
archive.with_suffix(archive.suffix + '.sha256').write_text(f'{digest}  {archive.name}\n')
print(archive)
print(digest)
