#!/usr/bin/env python3
"""Source-only path transport and repeat algebra controls, safe under python -O."""
from dataclasses import replace
import json
import struct
from oracles.optimal_paths_wide import Request, parse_packet, distance, next_repeats, valid_match, require


def check():
    base = Request(b'a' * 32, ((1, ((4, 3), (4, 8))), (9, ((12, 8),))), target=8)
    positives = [base, Request(bytes(range(9)), ()), replace(base, min_match=4, scripts=((1, ((4, 4), (4, 8))),)),
                 Request(b'a' * 8192, tuple((i + 1, tuple((4, j + 3) for j in range(8))) for i in range(64)), target=4095, literal_mode=1)]
    for request in positives:
        require(parse_packet(request.packet()) == request, 'roundtrip')
    invalid = [replace(base, source=b'a' * 8), replace(base, source=b'a' * 8193), replace(base, source=bytearray(base.source)),
               replace(base, scripts=[]), replace(base, min_match=2), replace(base, min_match=8), replace(base, min_match=True),
               replace(base, target=-1), replace(base, target=4096), replace(base, literal_mode=2),
               replace(base, scripts=((0, ((4, 3),)),)), replace(base, scripts=((25, ((4, 3),)),)),
               replace(base, scripts=((1, ()),)), replace(base, scripts=((1, ((4, 3),) * 9),)),
               replace(base, scripts=((1, ((0, 3),)),)), replace(base, scripts=((1, ((5, 3),)),)),
               replace(base, scripts=((1, ((4, 2),)),)), replace(base, scripts=((1, ((4, 32),)),)),
               replace(base, scripts=((1, ((4, 4), (4, 3))),)), replace(base, scripts=((1, ((4, 3), (4, 3))),)),
               replace(base, scripts=((1, ((4, 3),)), (1, ((4, 3),)))),
               replace(base, source=bytes(range(32))), replace(base, scripts=((True, ((4, 3),)),))]
    rejected = 0
    for request in invalid:
        try:
            request.packet()
        except ValueError:
            rejected += 1
        else:
            raise ValueError('invalid request accepted')
    packet = base.packet()
    packets = [packet[:i] for i in (0, 7, 27, 35, len(packet) - 1)] + [packet + b'\0', b'BADMAGIC' + packet[8:]]
    for offset, value in [(8, 8193), (12, 65), (16, 2), (20, 4096), (24, 2), (28 + len(base.source) + 4, 9)]:
        forged = bytearray(packet)
        struct.pack_into('<I', forged, offset, value)
        packets.append(bytes(forged))
    for data in packets:
        try:
            parse_packet(data)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('malformed packet accepted')
    reps = (5, 9, 13)
    require([distance(i, reps, False) for i in range(1, 5)] == [5, 9, 13, 1], 'normal distances')
    require([distance(i, reps, True) for i in range(1, 5)] == [9, 13, 4, 1], 'zero-literal distances')
    require([next_repeats(i, reps, False) for i in range(1, 5)] == [(5, 9, 13), (9, 5, 13), (13, 5, 9), (1, 5, 9)], 'normal publication')
    require([next_repeats(i, reps, True) for i in range(1, 5)] == [(9, 5, 13), (13, 5, 9), (4, 5, 9), (1, 5, 9)], 'zero-literal publication')
    for args in [(b'a' * 16, 1, 2, 3, (1, 4, 8), False), (b'a' * 16, 1, 3, 3, (1, 4, 8), True)]:
        try:
            valid_match(*args)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('unavailable repeat accepted')
    return dict(status='passed', positives=len(positives), rejected=rejected, child_processes=0)


if __name__ == '__main__':
    print(json.dumps(check(), sort_keys=True))
