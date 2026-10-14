#!/usr/bin/env python3
"""Validate frozen tiny all-match inputs without executing a codec or children."""
import argparse
from dataclasses import replace
import json
from pathlib import Path
import struct
import sys
from oracles import optimal_match_recipes as recipes
from oracles.optimal_matches import Request, MAX_PACKET, parse_packet, require

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'tests/fixtures/reference-optimal-match-recipes/recipes.json'


def check(write=False):
    children = []

    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')

    sys.addaudithook(guard)
    values = recipes.recipes()
    manifest = recipes.manifest()
    require((manifest['recipe_count'], manifest['source_bytes'], manifest['queries'])
            == (29, 71072, 273), 'frozen inventory')
    for value in values:
        require(parse_packet(value.request.packet()) == value.request, 'exact input round trip')
    require(len(values[-1].request.packet()) == MAX_PACKET, 'maximum packet is admitted')
    require({v.request.min_match for v in values} == {3, 4, 5, 6, 7}, 'minimum widths')
    require({v.request.target for v in values} == {0, 1, 2, 3, 4, 16, 4095, 4096, 131072}, 'target boundaries')
    minimal = Request(b'abcdefgh', ((0, 0, 0, 0, 0, 4),))
    require(len(minimal.packet()) == 76 and parse_packet(minimal.packet()) == minimal,
            'minimum packet is admitted')
    for beat in (4, 8):
        request = replace(minimal, queries=((0, 0, 1, 2, 1, beat),))
        require(parse_packet(request.packet()) == request, 'repeat pointer and full-tail bounds')
    rejected = []

    def reject(label, action):
        try:
            action()
        except ValueError:
            rejected.append(label)
        else:
            raise ValueError('accepted malformed input: ' + label)

    fields = [('window_log', 10, 17), ('hash_log', 6, 12), ('chain_log', 6, 12),
              ('search_log', 1, 8), ('min_match', 3, 7), ('target', 0, 131072)]
    for field, low, high in fields:
        for value in (low - 1, high + 1, True, float(low)):
            reject(f'{field}:{value!r}', lambda f=field, v=value: replace(minimal, **{f: v}).validate())
    for value in (b'', b'1234567', b'a' * 16385, bytearray(b'abcdefgh'), 'abcdefgh'):
        reject('source extent/type', lambda v=value: replace(minimal, source=v).validate())
    for value in ((), [], ((0, 0, 0, 0, 0, 4),) * 65,
                  ((0, 0, 0, 0, 0),), ([0, 0, 0, 0, 0, 4],),
                  ((0, 0, 0, 0, 0, 4),) * 2):
        reject('query count/shape/order', lambda v=value: replace(minimal, queries=v).validate())
    for index, invalid in enumerate(((-1, 1, True, 0.0), (-1, 3, True, 0.0),
                                    (-1, 3, True, 0.0), (-1, 3, True, 0.0),
                                    (-1, 2, True, 0.0), (3, 9, True, 4.0))):
        for value in invalid:
            query = list(minimal.queries[0])
            query[index] = value
            reject(f'query word {index}:{value!r}',
                   lambda q=tuple(query): replace(minimal, queries=(q,)).validate())
    for minimum, h3 in ((3, 0), (3, 5), (3, 13), (3, True), (4, 6), (4, False)):
        reject('short-key ownership', lambda m=minimum, h=h3:
               replace(minimal, min_match=m, hash3_log=h).validate())
    packet = minimal.packet()
    for n in range(len(packet)):
        reject(f'truncated:{n}', lambda n=n: parse_packet(packet[:n]))
    for data in (packet + b'\0', b'BADMAGIC' + packet[8:], bytearray(packet), 'not bytes', b'\0' * (MAX_PACKET + 1)):
        reject('packet identity/type/extent', lambda d=data: parse_packet(d))
    for word, value in ((0, 7), (0, 16385), (0, 9), (1, 0), (1, 65),
                        (2, 9), (3, 13), (4, 5), (5, 0), (6, 2), (7, 131073), (8, 6)):
        changed = bytearray(packet)
        struct.pack_into('<I', changed, 8 + 4 * word, value)
        reject(f'wire header:{word}', lambda d=bytes(changed): parse_packet(d))
    for word, value in ((0, 1), (1, 3), (2, 0xffffffff), (3, 3), (4, 2), (5, 9)):
        changed = bytearray(packet)
        struct.pack_into('<I', changed, 52 + 4 * word, value)
        reject(f'wire query:{word}', lambda d=bytes(changed): parse_packet(d))
    for change in (dict(name=''), dict(name='om_BAD'), dict(name='om_' + 'x' * 61),
                   dict(name=1), dict(request=None), dict(hypothesis=''),
                   dict(hypothesis='x' * 257), dict(hypothesis=True)):
        reject('recipe metadata', lambda c=change: replace(values[0], **c).metadata())
    canonical = (json.dumps(manifest, sort_keys=True, indent=2) + '\n').encode()
    if write:
        require(not DEST.exists(), 'fresh frozen manifest')
        DEST.parent.mkdir(parents=True, exist_ok=True)
        DEST.write_bytes(canonical)
    require(DEST.is_file() and not DEST.is_symlink() and DEST.read_bytes() == canonical,
            'exact frozen manifest')
    require(not children, 'no child attempts')
    print(json.dumps(dict(status='passed', recipes=29, source_bytes=71072, queries=273,
                          malformed_rejections=len(rejected), child_attempts=len(children)), indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    check(parser.parse_args().write)
