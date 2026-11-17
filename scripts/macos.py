"""Shared minimum macOS contract for packaging and verification."""
import re

MIN_MACOS = '14.0'


def validate_minimum_os(load_commands):
    blocks = re.findall(r'\bcmd LC_(BUILD_VERSION|VERSION_MIN_MACOSX)\n(.*?)(?=\nLoad command|\Z)', load_commands, re.S)
    if not blocks:
        raise ValueError('Mach-O has no recognized minimum macOS version')
    maximum = tuple(int(n) for n in MIN_MACOS.split('.')) + (0,)
    for kind, block in blocks:
        if kind == 'BUILD_VERSION' and not re.search(r'^\s*platform (?:1|macos)\s*$', block, re.M):
            raise ValueError('Mach-O is not a macOS binary')
        field = 'minos' if kind == 'BUILD_VERSION' else 'version'
        match = re.search(r'^\s*'+field+r' (\d+\.\d+(?:\.\d+)?)\s*$', block, re.M)
        if not match:
            raise ValueError('Cannot read minimum macOS version')
        parts = tuple(int(n) for n in match[1].split('.'))
        minimum = parts + (0,) * (3-len(parts))
        if minimum > maximum:
            raise ValueError(f'Binary requires macOS {match[1]}; project promises {MIN_MACOS}+. Review compatibility before release.')
