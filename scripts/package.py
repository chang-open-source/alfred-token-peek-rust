#!/usr/bin/env python3
"""Build one importable macOS workflow for Apple Silicon and Intel."""
import hashlib
import os
from pathlib import Path
import shutil
import struct
import subprocess
import zlib
import vendor
from macos import MIN_MACOS

ROOT = Path(__file__).resolve().parents[1]

def icon(path, color):
    # Original geometric meter icon; no third-party branding assets.
    size = 128
    data = bytearray()
    for y in range(size):
        data.append(0)
        for x in range(size):
            rgb = (25, 29, 39)
            for left, top in [(27, 70), (53, 47), (79, 24)]:
                if left <= x < left + 20 and top <= y < 103:
                    rgb = color
            data.extend((*rgb, 255))
    def chunk(kind, body):
        return struct.pack('>I', len(body)) + kind + body + struct.pack('>I', zlib.crc32(kind + body))
    path.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size,size,8,6,0,0,0)) + chunk(b'IDAT',zlib.compress(bytes(data))) + chunk(b'IEND',b''))

def main():
    vendor.main()
    env = dict(os.environ, MACOSX_DEPLOYMENT_TARGET=MIN_MACOS)
    targets = ['aarch64-apple-darwin', 'x86_64-apple-darwin']
    for target in targets:
        subprocess.run(['cargo','build','--release','--locked','--target',target],cwd=ROOT,env=env,check=True)
    dist = ROOT / 'dist'
    stage = dist / 'workflow'
    if stage.exists(): shutil.rmtree(stage)
    stage.mkdir(parents=True)
    subprocess.run(['lipo','-create',*[str(ROOT/'target'/t/'release/token-peek') for t in targets],'-output',str(stage/'token-peek')],check=True)
    shutil.copytree(ROOT/'vendor/runtime',stage/'vendor',symlinks=True)
    for path in (ROOT/'workflow').iterdir():
        if path.is_file(): shutil.copy2(path, stage/path.name)
    for name in ['README.md','README.zh.md','LICENSE','CHANGELOG.md']:
        shutil.copy2(ROOT/name,stage/name)
    shutil.copy2(ROOT/'vendor/manifest.json',stage/'vendor/manifest.json')
    (stage/'docs').mkdir()
    for name in ['verification.md', 'releasing.md']:
        shutil.copy2(ROOT/'docs'/name,stage/'docs'/name)
    (stage/'icons').mkdir()
    icon(stage/'icon.png',(98,205,181))
    icon(stage/'icons/codex.png',(98,205,181))
    icon(stage/'icons/claude.png',(219,160,124))
    # Ship licenses for the exact locked Rust dependencies as well.
    packages=set()
    for target in targets:
        tree=subprocess.check_output(['cargo','tree','--locked','--offline','--target',target,'--prefix','none','--format','{p}'],cwd=ROOT,text=True)
        for line in tree.splitlines():
            name,version,*_=line.split()
            if name != 'token-peek': packages.add(f'{name}-{version.removeprefix("v")}')
    registry=Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))/'registry/src'
    notices=stage/'licenses/rust'
    notices.mkdir(parents=True)
    for label in sorted(packages):
        matches=list(registry.glob(f'*/{label}'))
        if not matches: raise SystemExit(f'Missing dependency sources: {label}; run cargo fetch --locked')
        source=matches[0]
        licenses=[p for p in source.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE','COPYING','NOTICE'))]
        dest=notices/label
        dest.mkdir()
        for path in licenses: shutil.copy2(path,dest/path.name)
        if not licenses: raise SystemExit(f'Missing license files for {label}')
    output=dist/'alfred-token-peek.alfredworkflow'
    output.unlink(missing_ok=True)
    subprocess.run(['zip','-qry',str(output),'.'],cwd=stage,check=True)
    digest=hashlib.sha256(output.read_bytes()).hexdigest()
    (dist/'SHA256SUMS').write_text(f'{digest}  {output.name}\n')
    print(f'Packaged {output} ({output.stat().st_size/1024/1024:.1f} MiB)\nSHA256 {digest}')

if __name__ == '__main__': main()
