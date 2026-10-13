"""Bounded v1 binary-tree requests; source recipes contain no predicted answers.

No codec invocation or observation synthesis lives here. The wire contract is
specified in docs/reference-tree-oracle.md against pinned Zstandard v1.5.7.
"""
from dataclasses import dataclass
import hashlib
import struct

REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAGIC = b'ZTREE01\0'
MAX_SOURCE = 131072
MAX_OPERATIONS = 256
MAX_PACKET = 133200


def require(value, reason):
    if not value:
        raise ValueError(reason)


def integer(value, low, high, reason):
    require(type(value) is int and low <= value <= high, reason)
    return value


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fnv(data):
    value = 0xcbf29ce484222325
    for byte in data:
        value = ((value ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
    return f'{value:016x}'


@dataclass(frozen=True)
class Request:
    source: bytes
    queries: tuple[int, ...]
    window_log: int = 10
    hash_log: int = 6
    chain_log: int = 6
    search_log: int = 1
    min_match: int = 4

    def packet(self):
        require(type(self.source) is bytes, 'owned source bytes')
        integer(len(self.source), 8, MAX_SOURCE, 'source extent')
        require(type(self.queries) is tuple, 'immutable query schedule')
        integer(len(self.queries), 1, MAX_OPERATIONS, 'operation count')
        integer(self.window_log, 10, 17, 'window log')
        integer(self.hash_log, 6, 12, 'hash log')
        integer(self.chain_log, 6, 12, 'chain log')
        integer(self.search_log, 1, 8, 'search log')
        integer(self.min_match, 3, 7, 'minimum match')
        previous = -1
        for query in self.queries:
            integer(query, 0, len(self.source) - 8, 'inclusive hash query bound')
            require(query > previous, 'strictly increasing queries')
            previous = query
        words = (1, 80, len(self.source), len(self.queries), self.window_log,
                 self.hash_log, self.chain_log, self.search_log, self.min_match,
                 0, 6, 0, 0, 0, 0, 0, 0, 0)
        data = MAGIC + struct.pack('<18I', *words)
        data += b''.join(struct.pack('<II', 1, q) for q in self.queries)
        data += self.source
        require(len(data) <= MAX_PACKET, 'packet bound')
        return data

    def metadata(self):
        packet = self.packet()
        return dict(source_bytes=len(self.source), source_sha256=sha(self.source),
                    source_fnv1a64=fnv(self.source), queries=list(self.queries),
                    parameters=[self.window_log, self.chain_log, self.hash_log,
                                self.search_log, self.min_match, 0, 6],
                    packet_bytes=len(packet), packet_sha256=sha(packet))


def decode_packet(data):
    require(type(data) is bytes and 96 <= len(data) <= MAX_PACKET, 'packet extent')
    require(data[:8] == MAGIC, 'protocol magic')
    words = struct.unpack_from('<18I', data, 8)
    require(words[:2] == (1, 80), 'version and header size')
    n = integer(words[2], 8, MAX_SOURCE, 'source extent')
    count = integer(words[3], 1, MAX_OPERATIONS, 'operation count')
    require(words[9:] == (0, 6, 0, 0, 0, 0, 0, 0, 0), 'fixed and reserved fields')
    end = 80 + 8 * count
    require(len(data) == end + n, 'exact operation/source extent')
    queries = []
    for offset in range(80, end, 8):
        kind, query = struct.unpack_from('<II', data, offset)
        require(kind == 1, 'find-only operation')
        queries.append(query)
    request = Request(data[end:], tuple(queries), *words[4:9])
    require(request.packet() == data, 'canonical packet round trip')
    return request


def noise(size, seed=0x5eedb71220261013):
    """Fixed xorshift64 input generator, unrelated to tree selection."""
    state = seed
    result = bytearray()
    for _ in range(size):
        state ^= (state << 13) & ((1 << 64) - 1)
        state ^= state >> 7
        state ^= (state << 17) & ((1 << 64) - 1)
        result.append(state & 255)
    return bytes(result)


def recipes():
    """Coverage labels express intended input families, not observed C branches."""
    cases = []

    def add(name, source, queries, **parameters):
        request = Request(source, tuple(queries), **parameters)
        request.packet()
        cases.append((name, request))

    for size in (8, 9, 15, 16, 17, 31, 32, 33):
        add(f'noise-tail-{size}', noise(size), sorted({0, size - 8}))
    for minimum in (3, 4, 5, 6, 7):
        add(f'equal-end-min-{minimum}', b'a' * 128,
            (0, 1, 2, 3, 31, 32, 33, 63, 64, 119, 120), min_match=minimum)
    for budget in (1, 2, 4, 8):
        add(f'unsorted-budget-{budget}', b'abcd' * 96,
            (0, 32, 64, 96, 128, 192, 256, 320, 376), search_log=budget)
    for chain in (6, 7, 8, 12):
        ring = 1 << (chain - 1)
        size = 2 * ring + 24
        add(f'ring-noise-{chain}', noise(size),
            (0, ring - 1, ring, ring + 1, 2 * ring - 1, 2 * ring, size - 8),
            chain_log=chain, search_log=4, window_log=17)
    for order in ('ascending', 'descending', 'alternating'):
        values = list(range(64))
        if order == 'descending':
            values.reverse()
        elif order == 'alternating':
            values = [i // 2 if i % 2 == 0 else 63 - i // 2 for i in range(64)]
        source = b''.join(b'same' + bytes([value]) + b'XYZ' for value in values)
        source += source[:64]
        add('lexical-' + order, source, range(0, len(source) - 7, 8),
            chain_log=8, search_log=4)
    for window in (10, 11):
        prefix = noise(1 << window)
        source = prefix + prefix[:128] + b'q' * 16
        add(f'window-floor-{window}', source,
            (0, 1, 32, 64, (1 << window) - 1, 1 << window,
             (1 << window) + 1, len(source) - 8),
            chain_log=12, window_log=window, search_log=8)
    for width in (6, 9, 12):
        add(f'hash-layout-{width}', noise(520), range(0, 513, 4),
            hash_log=width, chain_log=8, search_log=4)
    add('maximum-operation-count', noise(263), range(256))
    require(len(cases) == 30 and len({n for n, _ in cases}) == 30, 'tiny recipe inventory')
    return cases


def manifest():
    return dict(schema_version=1, protocol='ZTREE01', upstream_revision=REVISION,
                qualification='Inputs only. Family names are coverage intentions; no C observations or branch witnesses are claimed.',
                recipes=[dict(name=name, **request.metadata(), source_hex=request.source.hex())
                         for name, request in recipes()])
