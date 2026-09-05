#!/usr/bin/env python3
"""Version checks and deterministic release metadata. Never publishes or tags."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib

TARGETS = (
    ('x86_64-unknown-linux-gnu', 'tar.gz'),
    ('aarch64-apple-darwin', 'tar.gz'),
    ('x86_64-apple-darwin', 'tar.gz'),
    ('x86_64-pc-windows-msvc', 'zip'),
)


def package_version(root, tag=''):
    package = tomllib.loads((root / 'Cargo.toml').read_text())['package']
    version = package['version']
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?', version):
        raise ValueError('Cargo package version must be a literal semantic version')
    locked = tomllib.loads((root / 'Cargo.lock').read_text())['package']
    own = [entry for entry in locked if entry['name'] == package['name'] and 'source' not in entry]
    if len(own) != 1 or own[0]['version'] != version:
        raise ValueError('Cargo.toml and Cargo.lock package versions disagree')
    if tag and tag != f'v{version}':
        raise ValueError(f'tag {tag!r} must exactly match v{version}')
    return version


def closed_beads(text):
    return {row['id']: row for line in text.splitlines() if line.strip()
            for row in [json.loads(line)] if row['status'] == 'closed'}


def changelog(current, previous, tag):
    closed = closed_beads(current)
    prior = closed_beads(previous)
    entries = [closed[key] for key in sorted(closed.keys() - prior.keys())]
    lines = [f'# {tag}', '', 'Completed beads since the previous release tag:', '']
    for row in entries:
        title = ' '.join(row['title'].split())
        # Bead titles are data, never HTML or executable workflow expressions.
        title = title.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
        lines.append(f"- `{row['id']}`: {title}")
    if not entries:
        lines.append('No newly closed beads in the committed bead snapshot.')
    return '\n'.join(lines) + '\n'


def previous_snapshot(root, previous_tag):
    if not previous_tag:
        # HEAD^ excludes the candidate tag itself; the first release has no baseline.
        probe = subprocess.run(['git', 'describe', '--tags', '--abbrev=0', '--match', 'v[0-9]*', 'HEAD^'],
                               cwd=root, text=True, capture_output=True, check=False)
        previous_tag = probe.stdout.strip() if probe.returncode == 0 else ''
    if not previous_tag:
        return ''
    return subprocess.run(['git', 'show', f'{previous_tag}:.beads/issues.jsonl'],
                          cwd=root, text=True, capture_output=True, check=True).stdout


def copy_legion_proof(source, dist):
    if not source.is_file():
        return False
    report = json.loads(source.read_text(encoding='utf-8-sig'))
    if report.get('fixtureOnly') or report.get('skipOffice') is not False or report.get('status') != 'passed':
        raise ValueError('Legion report must be real passed desktop Office proof, not a fixture or SkipOffice run')
    if report.get('schemaVersion') != 'ooxml-cli.legion-proof.v1':
        raise ValueError('unrecognized Legion proof schema')
    shutil.copyfile(source, dist / 'legion-proof.json')
    return True


def verify_archives(dist, tag):
    expected = {f'ooxml-{tag}-{target}.{suffix}' for target, suffix in TARGETS}
    found = {path.name for path in dist.iterdir() if path.is_file()}
    if found != expected:
        raise ValueError(f'expected exactly four release archives; missing={sorted(expected-found)}, unexpected={sorted(found-expected)}')
    if any((dist / name).stat().st_size == 0 for name in expected):
        raise ValueError('release archives must not be empty')


def write_checksums(dist):
    manifest = ''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n'
                       for path in sorted(dist.iterdir()) if path.is_file() and path.name != 'SHA256SUMS')
    (dist / 'SHA256SUMS').write_text(manifest, encoding='utf-8', newline='\n')


def assemble(root, dist, binary, tag, previous_tag='', proof=None):
    version = package_version(root, tag)
    verify_archives(dist, tag)
    actual = json.loads(subprocess.run([str(binary), '--json', 'version'],
                                      check=True, capture_output=True, text=True).stdout)
    if actual.get('version') != version:
        raise ValueError('packaged binary version disagrees with Cargo package')
    capabilities = subprocess.run([str(binary), '--json', 'capabilities'],
                                  check=True, capture_output=True).stdout
    parsed = json.loads(capabilities)
    if not isinstance(parsed, dict) or not parsed:
        raise ValueError('capabilities must be a nonempty JSON object')
    # Preserve the CLI contract bytes (including its terminal newline).
    (dist / 'capabilities.json').write_bytes(capabilities)
    notes = changelog((root / '.beads/issues.jsonl').read_text(), previous_snapshot(root, previous_tag), tag)
    has_proof = copy_legion_proof(proof or root / 'proof/legion-summary.json', dist)
    notes += '\nDesktop Office proof: ' + ('attached as `legion-proof.json`.\n' if has_proof else 'not attached; no compatibility claim is made by this pipeline.\n')
    (dist / 'CHANGELOG.md').write_text(notes, encoding='utf-8', newline='\n')
    write_checksums(dist)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['version', 'assemble'])
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--tag', default='')
    parser.add_argument('--github-output', type=Path)
    parser.add_argument('--dist', type=Path)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--previous-tag', default='')
    parser.add_argument('--legion-proof', type=Path)
    args = parser.parse_args()
    version = package_version(args.root, args.tag)
    tag = f'v{version}'
    if args.command == 'assemble':
        if args.dist is None or args.binary is None:
            parser.error('assemble requires --dist and --binary')
        assemble(args.root, args.dist, args.binary.resolve(), tag, args.previous_tag, args.legion_proof)
    if args.github_output:
        with args.github_output.open('a', encoding='utf-8', newline='\n') as output:
            output.write(f'tag={tag}\n')
    print(json.dumps({'version': version, 'tag': tag}, sort_keys=True))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f'release-assets: {error}', file=sys.stderr)
        sys.exit(1)
