#!/usr/bin/env python3
"""Bind all original retained Optimal native artifacts to additive policy zero."""
from pathlib import Path
import argparse
import hashlib
import json
import sys
import tarfile
sys.dont_write_bytecode = True

from oracles.tree_validation import parse, same
from oracles.ultra_retained import MAGIC, require

ROOT = Path(__file__).resolve().parents[1]
ARCHIVE_SHA = '750e3a3624c78b9d189ab93249085f96736667d25f857156ae1e065e05e4e985'


def sha_file(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        while part := stream.read(65536):
            digest.update(part)
    return digest.hexdigest()


def check(capture):
    children = []
    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')
    sys.addaudithook(guard)
    capture = capture.resolve(strict=True)
    evidence = parse((capture/'evidence.json').read_bytes())
    require(evidence['status'] == 'passed' and evidence['source_unchanged'] is True
            and evidence['audit']['recipes'] == 42, 'completed independently audited successor')
    report = parse((ROOT/'docs/evidence/2026-10-20-optimal-retained-frames-capture.json').read_bytes())
    archive = ROOT/'docs/evidence'/report['archive']
    require(report['archive_sha256'] == sha_file(archive) == ARCHIVE_SHA
            and archive.stat().st_size == report['archive_bytes'], 'pinned predecessor archive')
    inventory = {row['path']: row for row in report['files']}
    require(len(inventory) == len(report['files']), 'unique predecessor inventory')
    seen = set()
    frames = ledgers = packets = transcripts = events = 0
    with tarfile.open(archive, 'r|gz') as tar:
        for member in tar:
            require(member.isfile() and member.name in inventory and member.name not in seen,
                    'exact regular predecessor member')
            seen.add(member.name)
            old = tar.extractfile(member)
            identity = inventory[member.name]
            require(member.size == identity['bytes'] and 0 <= member.size <= 256 << 20,
                    'bounded predecessor member')
            digest = hashlib.sha256()
            name = member.name
            if name.startswith('logs/') and '-run-wide_' in name and name.endswith('.stdout'):
                new_name = name.replace('-run-wide_', '-run-ur_p0_wide_')
                path = capture/new_name
                require(path.is_file() and not path.is_symlink(), 'regular successor transcript')
                command = next(c for c in evidence['commands'] if c['stdout']['path'] == new_name)
                require(sha_file(path) == command['stdout']['sha256']
                        and path.stat().st_size == command['stdout']['bytes'], 'bound successor transcript')
                with path.open('rb') as new:
                    while line := old.readline((2 << 20)+1):
                        require(len(line) <= 2 << 20 and line.endswith(b'\n'), 'bounded predecessor row')
                        digest.update(line)
                        actual = parse(new.readline((2 << 20)+1))
                        require(type(actual.get('price_policy')) is int and actual.pop('price_policy') == 0,
                                'exact additive policy-zero tag')
                        if actual['record'] == 'metadata':
                            require(actual['protocol'] == 'ZURTS01', 'successor protocol')
                            actual['protocol'] = 'ZOPRS03'
                        elif actual['record'] == 'event':
                            require(actual.pop('alternative') is None, 'no policy-zero alternative')
                            events += 1
                        same(actual, parse(line), 'unchanged complete native predecessor state')
                    require(new.read(1) == b'', 'exact successor transcript EOF')
                transcripts += 1
            else:
                data = old.read()
                digest.update(data)
                if name.startswith('artifacts/'):
                    mode, case = name.split('/')[1:]
                    new_name = 'artifacts/'+mode+'/ur_p0_'+case
                    path = capture/new_name
                    require(path.is_file() and not path.is_symlink() and path.read_bytes() == data,
                            'unchanged predecessor frame/ledger bytes')
                    require(evidence['artifacts'][new_name] == dict(bytes=len(data), sha256=identity['sha256']),
                            'bound successor artifact')
                    frames += name.endswith('.zst')
                    ledgers += name.endswith('.json')
                elif name.startswith('requests/wide_'):
                    path = capture/'requests'/('ur_p0_'+Path(name).name)
                    require(path.is_file() and not path.is_symlink()
                            and path.read_bytes() == MAGIC+b'\0'*4+data,
                            'exact unchanged nested predecessor packet')
                    packets += 1
            require(digest.hexdigest() == identity['sha256'], 'exact predecessor member hash')
    require(seen == set(inventory) and (frames, ledgers, packets, transcripts, events)
            == (76, 76, 19, 38, 269284) and not children, 'complete predecessor reconciliation')
    return dict(status='passed', frames=frames, allocation_ledgers=ledgers, packets=packets,
                transcripts=transcripts, events=events, child_processes=0,
                predecessor_archive_sha256=ARCHIVE_SHA,
                successor_evidence_sha256=sha_file(capture/'evidence.json'),
                checker_sha256=sha_file(Path(__file__)),
                qualification='All original normal/sanitized and two-padding-pass retained Optimal frames and allocation ledgers remain byte-identical. Every complete transcript differs only by declared additive policy/alternative tags and protocol identity. All nested old packets are intact. No native rerun is inferred.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    print(json.dumps(check(parser.parse_args().capture), sort_keys=True, indent=2))
