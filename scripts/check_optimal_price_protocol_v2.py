#!/usr/bin/env python3
"""Check source-only Optimal price transport and malformed schedules; no children."""
import json
import struct
import sys

from oracles.optimal_prices_v2 import MAX_SOURCE, Request, parse_packet, require


def main():
    children = []

    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')

    sys.addaudithook(guard)
    init = (0, 0, 9, 0, 0, 0)
    update = (1, 0, 9, 1, 3, 0)
    refresh = (2, 0, 0, 0, 0, 0)
    query = (4, 0, 9, 131072, 0xffffffff, 131072)
    positive = [
        Request(b'a' * 9, (init, (5, 0, 0, 1, 3, 8192), refresh, query)),
        Request(b'', ((0, 0, 0, 0, 0, 0),)),
        Request(b'a' * 9, (init, query)),
        Request(b'a' * 9, (init, update, refresh, query)),
        Request(b'a' * 9, (init, update, (3, 9, 0, 0, 0, 0), query)),
        Request(b'a' * 9, (init, query), 1),
        Request(b'a' * MAX_SOURCE, ((0, 0, MAX_SOURCE, 0, 0, 0),) + (refresh,) * 127),
        Request(b'a' * MAX_SOURCE, ((0, 0, MAX_SOURCE, 0, 0, 0),)
                + ((1, 0, MAX_SOURCE - 1, 0xffffffff, MAX_SOURCE, 0),) * 32
                + (refresh, (4, 0, MAX_SOURCE, MAX_SOURCE, 0xffffffff, MAX_SOURCE))),
        Request(b'a', ((0, 0, 0, 0, 0, 0), (3, 0, 1, 0, 0, 0),
                       (4, 0, 0, 0, 1, 3))),
        Request(b'a', ((0, 0, 0, 0, 0, 0), (3, 0, 1, 0, 0, 0),
                       (4, 0, 1, 0, 1, 3)), 1),
    ]
    for request in positive:
        require(parse_packet(request.packet()) == request, 'positive exact round trip')
    rejects = []

    def reject(name, callback):
        try:
            callback()
        except ValueError:
            rejects.append(name)
        else:
            raise ValueError('accepted malformed input: ' + name)

    valid = positive[1].packet()
    mutations = {
        'truncated-header': valid[:19], 'truncated-operation': valid[:-1],
        'trailing-byte': valid + b'!', 'bad-magic': b'!' + valid[1:],
        'source-over-limit': valid[:8] + struct.pack('<I', MAX_SOURCE + 1) + valid[12:],
        'zero-count': valid[:12] + struct.pack('<I', 0) + valid[16:],
        'count-over-limit': valid[:12] + struct.pack('<I', 129) + valid[16:],
        'unknown-mode': valid[:16] + struct.pack('<I', 2) + valid[20:],
        'mutable-packet': bytearray(valid),
    }
    for name, data in mutations.items():
        reject(name, lambda data=data: parse_packet(data))
    invalid = {
        'batch-zero-count': Request(b'a' * 9, (init, (5, 0, 0, 1, 3, 0))),
        'batch-wide-count': Request(b'a' * 9, (init, (5, 0, 0, 1, 3, 8193))),
        'batch-literal-budget': Request(b'a' * MAX_SOURCE, ((0, 0, MAX_SOURCE, 0, 0, 0), (5, 0, MAX_SOURCE - 1, 1, 3, 33))),
        'batch-dirty-query': Request(b'a' * 9, (init, (5, 0, 0, 1, 3, 4096), query)),
        'mutable-source': Request(bytearray(b'a' * 9), (init,)),
        'mutable-schedule': Request(b'a' * 9, [init]),
        'mutable-operation': Request(b'a' * 9, (list(init),)),
        'boolean-mode': Request(b'a' * 9, (init,), True),
        'floating-mode': Request(b'a' * 9, (init,), 0.0),
        'boolean-word': Request(b'a' * 9, ((False, 0, 9, 0, 0, 0),)),
        'floating-word': Request(b'a' * 9, ((0, 0.0, 9, 0, 0, 0),)),
        'negative-word': Request(b'a' * 9, ((0, -1, 9, 0, 0, 0),)),
        'wide-word': Request(b'a' * 9, ((0, 0x100000000, 9, 0, 0, 0),)),
        'unknown-tag': Request(b'a' * 9, (init, (6, 0, 0, 0, 0, 0))),
        'missing-init': Request(b'a' * 9, (refresh,)),
        'double-init': Request(b'a' * 9, (init, init)),
        'init-range': Request(b'a' * 9, ((0, 1, 9, 0, 0, 0),)),
        'init-reserved': Request(b'a' * 9, ((0, 0, 9, 1, 0, 0),)),
        'update-zero-offset': Request(b'a' * 9, (init, (1, 0, 9, 0, 3, 0))),
        'update-short-match': Request(b'a' * 9, (init, (1, 0, 9, 1, 2, 0))),
        'update-full-block': Request(b'a' * MAX_SOURCE, ((0, 0, MAX_SOURCE, 0, 0, 0), (1, 0, MAX_SOURCE, 1, 3, 0))),
        'update-reserved': Request(b'a' * 9, (init, (1, 0, 9, 1, 3, 1))),
        'update-budget': Request(b'a' * 9, (init, (5, 0, 0, 1, 3, 8192), update)),
        'refresh-reserved': Request(b'a' * 9, (init, (2, 0, 0, 0, 0, 1))),
        'rescale-range': Request(b'a' * 9, (init, (3, 10, 0, 0, 0, 0))),
        'rescale-reserved': Request(b'a' * 9, (init, (3, 0, 9, 0, 1, 0))),
        'dirty-query': Request(b'a' * 9, (init, update, query)),
        'query-range': Request(b'a' * 9, (init, (4, 1, 9, 0, 1, 3))),
        'query-wide-LL': Request(b'a' * 9, (init, (4, 0, 0, MAX_SOURCE + 1, 1, 3))),
        'query-zero-offset': Request(b'a' * 9, (init, (4, 0, 0, 0, 0, 3))),
        'query-short-match': Request(b'a' * 9, (init, (4, 0, 0, 0, 1, 2))),
        'query-wide-match': Request(b'a' * 9, (init, (4, 0, 0, 0, 1, MAX_SOURCE + 1))),
        'dynamic-zero-literal-sum': Request(b'a', ((0, 0, 0, 0, 0, 0), (3, 0, 1, 0, 0, 0), (4, 0, 1, 0, 1, 3))),
    }
    for name, request in invalid.items():
        reject(name, request.packet)
    require(len(rejects) == 42 and not children, 'exact controls and no children')
    print(json.dumps({'status': 'passed', 'positive_round_trips': len(positive),
                      'rejections': rejects, 'child_attempts': len(children)}, indent=2))


if __name__ == '__main__':
    main()
