#!/usr/bin/env python3
"""Fail closed on release metadata and installer inventory; uses only the stdlib."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def version(root=ROOT):
    """Read the sole version source, rejecting versions unsuitable for installers."""
    value = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']['version']
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', value):
        raise ValueError('Expected a three-part numeric workspace version')
    return value


def release_notes(changelog, value):
    """Extract exactly one version section, failing if absent or empty."""
    sections = re.split(r'^## +', changelog, flags=re.MULTILINE)[1:]
    matches = [s.split('\n', 1)[1].strip() for s in sections
               if re.match(r'\[?' + re.escape(value) + r'\]?(?:\s|$)', s)]
    if len(matches) != 1 or not matches[0]:
        raise ValueError(f'Expected one nonempty CHANGELOG section for {value}')
    return matches[0] + '\n'


def metadata(tag=None):
    value = version()
    if tag is not None and tag != f'v{value}':
        raise ValueError('Tag must exactly match the root Cargo.toml version')
    notes = release_notes((ROOT / 'CHANGELOG.md').read_text(), value)
    Path('release-notes.md').write_text(notes)
    if output := os.environ.get('GITHUB_OUTPUT'):
        with open(output, 'a') as stream:
            stream.write(f'version={value}\n')
    print(value)


def collect(kind, source, destination, dry_run=False):
    """Stage exactly one selected installer, giving dry-run builds explicit names."""
    matches = sorted(Path(source).rglob(f'*.{kind}'))
    if len(matches) != 1:
        raise ValueError(f'Expected one {kind} in {source}, found {len(matches)}')
    arch = {'rpm': 'x86_64', 'dmg': 'universal', 'apk': 'arm64'}[kind]
    label = '-NOT-FOR-DISTRIBUTION' if dry_run else ''
    target = Path(destination)
    target.mkdir(parents=True, exist_ok=True)
    path = target / f'meshdrop-{version()}-{arch}{label}.{kind}'
    shutil.copyfile(matches[0], path)
    print(path)


def checksums(directory):
    """Require the complete three-installer set before calculating SHA256SUMS."""
    directory = Path(directory)
    paths = sorted(directory.iterdir())
    if sorted(p.suffix for p in paths) != ['.apk', '.dmg', '.rpm']:
        raise ValueError('Expected exactly one APK, DMG and RPM and no other files')
    lines = []
    for path in paths:
        if not path.is_file() or not path.name.startswith(f'meshdrop-{version()}-'):
            raise ValueError(f'Unexpected artifact: {path.name}')
        with path.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        lines.append(f'{digest}  {path.name}\n')
    (directory / 'SHA256SUMS').write_text(''.join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    sub.add_parser('metadata').add_argument('--tag')
    sub.add_parser('version')
    bundle = sub.add_parser('collect')
    bundle.add_argument('kind', choices=['rpm', 'dmg', 'apk'])
    bundle.add_argument('source')
    bundle.add_argument('destination')
    bundle.add_argument('--dry-run', action='store_true')
    sub.add_parser('checksums').add_argument('directory')
    args = parser.parse_args()
    if args.command == 'version':
        print(version())
    elif args.command == 'metadata':
        metadata(args.tag)
    elif args.command == 'collect':
        collect(args.kind, args.source, args.destination, args.dry_run)
    else:
        checksums(args.directory)


if __name__ == '__main__':
    main()
