#!/usr/bin/env python3
"""Reject altered frozen tree baselines without launching a child or codec."""
import copy
import json
from pathlib import Path
import shutil
import sys
import tempfile

import check_reference_tree_baseline as baseline


def main():
    attempts = []

    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            attempts.append(event)
            raise ValueError('child execution forbidden')

    sys.addaudithook(guard)
    root = baseline.ROOT
    expected, provenance = baseline.projection(root)
    baseline.require(baseline.audit(root) == 24, 'original positive control')
    rejected = []
    with tempfile.TemporaryDirectory(prefix='tree-baseline-controls-') as directory:
        isolated = Path(directory).resolve()
        archive = isolated / baseline.ARCHIVE
        archive.parent.mkdir(parents=True)
        archive.symlink_to(root / baseline.ARCHIVE)
        # The input checker intentionally rejects symlinks. Copy a real archive
        # once; no archived code is executed and no original is modified.
        archive.unlink()
        shutil.copyfile(root / baseline.ARCHIVE, archive)
        fixtures = isolated / baseline.COLLECTION
        fixtures.mkdir(parents=True)
        original = {p.name: p.read_bytes() for p in (root / baseline.COLLECTION).iterdir()}

        def reset():
            for p in fixtures.iterdir():
                p.unlink()
            for name, data in original.items():
                (fixtures / name).write_bytes(data)

        def reject(name, change):
            reset()
            change()
            try:
                baseline.audit(isolated)
            except (ValueError, OSError, KeyError, TypeError):
                rejected.append(name)
            else:
                raise ValueError('accepted altered baseline: ' + name)

        reject('frame-byte', lambda: (fixtures / 'frames.bin').write_bytes(b'!' + expected['frames.bin'][1:]))
        reject('truncated-pack', lambda: (fixtures / 'frames.bin').write_bytes(expected['frames.bin'][:-1]))
        reject('trailing-pack', lambda: (fixtures / 'frames.bin').write_bytes(expected['frames.bin'] + b'!'))
        reject('missing-case', lambda: (fixtures / 'expected.tsv').write_bytes(b'\n'.join(expected['expected.tsv'].splitlines()[:-1]) + b'\n'))
        for field in ('source_sha256', 'strategy', 'target_length', 'frame_offset',
                      'frame_sha256', 'attempts', 'owners_dropped'):
            def alter(field=field):
                lines = expected['expected.tsv'].decode().splitlines()
                row = lines[1].split('\t')
                index = baseline.HEADER.index(field)
                row[index] = '1' if row[index] == '0' else '0'
                lines[1] = '\t'.join(row)
                (fixtures / 'expected.tsv').write_text('\n'.join(lines) + '\n')
            reject('row-' + field, alter)
        for field, value in [('rows', True), ('pack_bytes', float(provenance['pack_bytes'])),
                             ('archive_sha256', '0' * 64), ('timed', True), ('configs', ['safe'])]:
            def alter(field=field, value=value):
                changed = copy.deepcopy(provenance)
                changed[field] = value
                (fixtures / 'provenance.json').write_text(json.dumps(changed))
            reject('provenance-' + field, alter)
        reject('extra-file', lambda: (fixtures / 'extra').write_bytes(b'x'))
        reject('missing-readme', lambda: (fixtures / 'README.md').unlink())
        reject('empty-readme', lambda: (fixtures / 'README.md').write_bytes(b''))
        reset()
        baseline.require(baseline.audit(isolated) == 24, 'restored isolated positive')
    baseline.require(not attempts and len(rejected) == 19, 'control count and child boundary')
    print(json.dumps({'status': 'passed', 'positive_audits': 2,
                      'forgery_rejections': rejected, 'child_attempts': len(attempts)}, indent=2))


if __name__ == '__main__':
    main()
