#!/usr/bin/env python3
"""Verify frozen tree inputs and malformed requests without executing a codec."""
import argparse
from dataclasses import replace
import hashlib
import json
from pathlib import Path
import struct
import sys

sys.dont_write_bytecode = True
from oracles import tree_direct as tree

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = 'tests/fixtures/reference-tree-tiny-recipes/recipes.json'
FIXTURE_SHA256 = '6d23c53ce5305ae0e9e9d43c5bd97bc6f3006f0207e75bebbf7b1843462fdff4'


def unique(pairs):
    result = {}
    for name, value in pairs:
        tree.require(name not in result, 'duplicate JSON key')
        result[name] = value
    return result


def reject_constant(value):
    raise ValueError('non-finite JSON constant: ' + value)


def parse(data):
    return json.loads(data, object_pairs_hook=unique, parse_constant=reject_constant)


def validate_manifest(value):
    # Serialized equality distinguishes bool/float from integer metadata.
    tree.require(json.dumps(value, sort_keys=True, allow_nan=False) ==
                 json.dumps(tree.manifest(), sort_keys=True, allow_nan=False),
                 'frozen tree source recipes differ')


def check(root=ROOT):
    path = root / FIXTURE
    tree.require(path.is_file() and not path.is_symlink() and
                 path.resolve().is_relative_to(root.resolve()), 'regular fixture')
    tree.require(path.stat().st_size <= 256 << 10, 'fixture size bound')
    data = path.read_bytes()
    tree.require(len(data) <= 256 << 10 and tree.sha(data) == FIXTURE_SHA256,
                 'fixture byte identity')
    validate_manifest(parse(data))
    cases = tree.recipes()
    for _, request in cases:
        tree.require(tree.decode_packet(request.packet()) == request,
                     'canonical source/request round trip')
    return dict(recipes=len(cases), source_bytes=sum(len(r.source) for _, r in cases),
                packet_bytes=sum(len(r.packet()) for _, r in cases),
                fixture_sha256=tree.sha(data), child_processes=0)


def controls():
    rejected = []

    def reject(name, call):
        try:
            call()
        except (ValueError, TypeError, struct.error):
            rejected.append(name)
        else:
            raise ValueError('accepted malformed input: ' + name)

    request = tree.Request(b'a' * 64, (0, 1, 56))
    packet = request.packet()
    for field, low, high in [('window_log', 10, 17), ('hash_log', 6, 12),
                             ('chain_log', 6, 12), ('search_log', 1, 8),
                             ('min_match', 3, 7)]:
        for value in (True, float(low), low - 1, high + 1):
            reject(f'{field}-{value!r}', lambda f=field, v=value:
                   replace(request, **{f: v}).packet())
    for name, value in [('empty', b''), ('short', b'a' * 7),
                        ('oversize', b'a' * (tree.MAX_SOURCE + 1)),
                        ('mutable', bytearray(b'a' * 64))]:
        reject('source-' + name, lambda v=value: replace(request, source=v).packet())
    for name, value in [('empty', ()), ('repeat', (1, 1)), ('backward', (2, 1)),
                        ('past-tail', (57,)), ('negative', (-1,)), ('boolean', (True,)),
                        ('float', (1.0,)), ('mutable', [0, 1]), ('too-many', tuple(range(257)))]:
        reject('queries-' + name, lambda v=value: replace(request, queries=v).packet())
    for offset, value in [(0, 0), (8, 2), (12, 84), (16, 7), (20, 0),
                           (44, 1), (48, 5), (52, 1), (56, 1), (60, 1),
                           (64, 1), (68, 1), (72, 1), (76, 1), (80, 2), (84, 57)]:
        forged = bytearray(packet)
        struct.pack_into('<I', forged, offset, value)
        reject('packet-word-' + str(offset), lambda b=bytes(forged): tree.decode_packet(b))
    for name, data in [('short', packet[:79]), ('truncated', packet[:-1]),
                       ('extra', packet + b'\0'), ('mutable', bytearray(packet)),
                       ('oversize', b'x' * (tree.MAX_PACKET + 1))]:
        reject('packet-' + name, lambda b=data: tree.decode_packet(b))
    for name, mutate in [
        ('schema-boolean', lambda d: d.update(schema_version=True)),
        ('missing-recipe', lambda d: d['recipes'].pop()),
        ('changed-source', lambda d: d['recipes'][0].update(source_hex='00')),
        ('changed-request', lambda d: d['recipes'][0]['parameters'].__setitem__(6, 5)),
        ('changed-packet-hash', lambda d: d['recipes'][0].update(packet_sha256='0' * 64)),
    ]:
        value = tree.manifest()
        mutate(value)
        reject('manifest-' + name, lambda v=value: validate_manifest(v))
    reject('duplicate-json', lambda: parse(b'{"a":1,"a":1}'))
    reject('non-finite-json', lambda: parse(b'{"a":NaN}'))
    # Both the largest admitted packet and the inclusive last query are positive
    # controls, not merely rejection tests near those boundaries.
    maximum = tree.Request(b'a' * tree.MAX_SOURCE, tuple(range(tree.MAX_OPERATIONS)),
                           window_log=17, hash_log=12, chain_log=12, search_log=8, min_match=7)
    tree.require(len(maximum.packet()) == tree.MAX_PACKET and
                 tree.decode_packet(maximum.packet()) == maximum, 'maximum packet round trip')
    tail = replace(request, queries=(56,))
    tree.require(tree.decode_packet(tail.packet()) == tail, 'inclusive tail round trip')
    return dict(positive=2, rejected=len(rejected), cases=rejected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    attempts = []

    def no_children(event, _args):
        if event in ('subprocess.Popen', 'os.system', 'os.fork', 'os.posix_spawn', 'os.exec', 'os.spawn'):
            attempts.append(event)
            raise RuntimeError('child process forbidden')

    sys.addaudithook(no_children)
    result = check()
    if args.self_test:
        result['controls'] = controls()
    tree.require(not attempts, 'no attempted child execution')
    result.update(optimized=not __debug__, source_sha256={n: hashlib.sha256((ROOT / n).read_bytes()).hexdigest()
                  for n in ('scripts/check_tree_recipes.py', 'scripts/oracles/tree_direct.py')})
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
