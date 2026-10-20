#!/usr/bin/env python3
"""Pure malformed two-pass packet and coherent lifecycle transcript controls."""
import copy
from dataclasses import replace
import json
import struct
import sys

sys.dont_write_bytecode = True
from oracles.ultra2_protocol import Request, MAX_PACKET, canonical, observations, parse_packet, validate_output
from oracles.ultra2_validation import SourceRequest
from oracles.ultra_scalar import need


def encoded(rows):
    return ''.join(canonical(row) + '\n' for row in rows).encode()


def check():
    children = []
    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.fork', 'os.posix_spawn', 'os.exec', 'os.spawn'):
            children.append(event)
            raise RuntimeError('protocol controls forbid child execution')
    sys.addaudithook(guard)
    request = Request(SourceRequest(b'abcd' * 16, (64,), min_match=3, target=128), True)
    packet = request.packet()
    need(parse_packet(packet) == request, 'positive packet')
    rejected = []
    def reject(name, call):
        try:call()
        except (ValueError, TypeError, KeyError):rejected.append(name)
        else:raise ValueError('accepted forgery: ' + name)
    for name, value in [('empty', b''), ('short', packet[:50]), ('extra', packet+b'!'),
                        ('truncated', packet[:-1]), ('magic', b'!'+packet[1:]),
                        ('oversize', b'0' * (MAX_PACKET+1)), ('bytearray', bytearray(packet))]:
        reject(name, lambda v=value: parse_packet(v))
    for offset, value in [(8, 6), (8, 16385), (12, 2), (12, 0xffffffff), (16, 9),
                          (16, 17), (20, 5), (20, 13), (24, 5), (24, 13), (28, 0),
                          (28, 7), (32, 2), (32, 8), (36, 4096), (40, 2)]:
        bad = packet[:offset]+struct.pack('<I', value)+packet[offset+4:]
        reject(f'field-{offset}-{value}', lambda v=bad: parse_packet(v))
    for value in (0, 1, None, 'true'):
        reject('seed-type-'+repr(value), lambda v=value: replace(request, seed=v).validate())
    for field in ('window_log', 'hash_log', 'chain_log', 'search_log', 'min_match', 'target', 'literal_mode'):
        reject('bool-'+field, lambda k=field: replace(request.original, **{k: True}).validate())
    good = list(observations(request))
    validate_output(encoded(good), request)
    for name in ('gate', 'phase', 'missing-reset', 'extra-reset', 'origin', 'frontier',
                 'discarded-repeats', 'caller-repeats', 'discarded-sequences', 'tail',
                 'coherent-frequency', 'cache', 'tables', 'short-cell', 'path-cell',
                 'sequence', 'padding', 'event-count', 'pass-count', 'boolean-index'):
        rows = copy.deepcopy(good)
        reset = next(r for r in rows if r['record'] == 'reset')
        event = next(r for r in rows if r.get('pass_name') == 'encoding' and r.get('stage') == 'query')
        if name == 'gate':next(r for r in rows if r['record'] == 'eligibility')['predicates'][0] = False
        elif name == 'phase':event['pass_name'] = 'seeding'
        elif name == 'missing-reset':rows.remove(reset)
        elif name == 'extra-reset':rows.insert(rows.index(reset), copy.deepcopy(reset))
        elif name == 'origin':reset['origin_after'] = reset['origin_before']
        elif name == 'frontier':reset['next_to_update'] = 2
        elif name == 'discarded-repeats':reset['discarded_repeats'][0] += 1
        elif name == 'caller-repeats':reset['caller_repeats'] = reset['discarded_repeats']
        elif name == 'discarded-sequences':reset['discarded_sequences'] += 1
        elif name == 'tail':reset['discarded_tail'] += 1
        elif name == 'coherent-frequency':
            for row in rows:
                if 'frequency' in row:
                    row['frequency']['tables']['ll'][0] += 1
                    row['frequency']['sums']['ll'] += 1
                if 'frequencies' in row:
                    row['frequencies']['ll'][0] += 1
                    row['sums']['ll'] += 1
        elif name == 'cache':reset['frequency']['cache']['ll'] += 1
        elif name == 'tables':reset['tables_preserved'] = False
        elif name == 'short-cell':event['search']['changes'].append([128, 2])
        elif name == 'path-cell':event['cells']['changes'].append([4098, [0]*7])
        elif name == 'sequence':next(r for r in rows if r['record'] == 'pass_end')['sequences'][0][3] += 1
        elif name == 'padding':event['padding_pass'] = 1
        elif name == 'event-count':rows[-1]['events'] += 1
        elif name == 'pass-count':rows[-1]['algorithm_passes'] += 1
        else:event['index'] = True
        reject(name, lambda r=rows: validate_output(encoded(r), request))
    raw = encoded(good)
    reject('duplicate-key', lambda: validate_output(raw.replace(b'"protocol":', b'"protocol":"forged","protocol":', 1), request))
    reject('trailing-record', lambda: validate_output(raw+b'{}\n', request))
    reject('missing-newline', lambda: validate_output(raw[:-1], request))
    validate_output(raw, request)
    need(not children, 'no children')
    return dict(status='passed', optimized=not __debug__, rejected=rejected,
                positives=3, child_processes=0,
                qualification='Packet boundaries and complete coherent two-pass transcript controls from '
                'independent source predictions; no native observer or public strategy admission.')


if __name__ == '__main__':
    print(json.dumps(check(), sort_keys=True, indent=2))
