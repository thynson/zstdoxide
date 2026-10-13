"""Source-only bounded requests for v1.5.7 optLevel=0 price observations.

No codec execution or expected-result synthesis. See the reviewed protocol in
 docs/reference-optimal-price-protocol.md for limits and lifecycle semantics.
"""
from dataclasses import dataclass
import hashlib
import struct

REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAGIC = b'ZOPPR01\0'
MAX_SOURCE = 131072
MAX_OPERATIONS = 128
MAX_PACKET = 134164
MAX_UPDATES = 32
MAX_UPDATED_LITERALS = 4194304


def require(value, reason):
    if not value:
        raise ValueError(reason)


def integer(value, low, high, reason):
    require(type(value) is int and low <= value <= high, reason)
    return value


def digest(data):
    return hashlib.sha256(data).hexdigest()


@dataclass(frozen=True)
class Request:
    source: bytes
    operations: tuple[tuple[int, int, int, int, int, int], ...]
    literal_mode: int = 0

    def validate(self):
        require(type(self.source) is bytes, 'owned source bytes')
        integer(len(self.source), 0, MAX_SOURCE, 'source extent')
        integer(self.literal_mode, 0, 1, 'literal mode')
        require(type(self.operations) is tuple, 'immutable operation schedule')
        integer(len(self.operations), 1, MAX_OPERATIONS, 'operation count')
        initialized = dirty = literal_mass = predefined = False
        updates = updated_literals = 0

        def span(offset, length):
            require(offset <= len(self.source) and length <= len(self.source) - offset,
                    'source range')

        for index, op in enumerate(self.operations):
            require(type(op) is tuple and len(op) == 6, 'six immutable operation words')
            for word in op:
                integer(word, 0, 0xffffffff, 'u32 operation word')
            tag, a, b, c, d, e = op
            integer(tag, 0, 4, 'operation tag')
            if tag == 0:
                require(index == 0 and not initialized, 'initialize exactly once first')
                span(a, b)
                require(c == d == e == 0, 'initialize reserved words')
                initialized = True
                literal_mass = b > 0 and self.literal_mode == 0
                predefined = b <= 8
            else:
                require(initialized, 'operation before initialization')
                if tag == 1:
                    span(a, b)
                    integer(b, 0, MAX_SOURCE - 1, 'update literal length')
                    integer(c, 1, 0xffffffff, 'update encoded offset')
                    integer(d, 3, MAX_SOURCE, 'update match length')
                    require(e == 0, 'update reserved word')
                    updates += 1
                    updated_literals += b
                    require(updates <= MAX_UPDATES and updated_literals <= MAX_UPDATED_LITERALS,
                            'update budget')
                    literal_mass |= b > 0 and self.literal_mode == 0
                    dirty = True
                elif tag == 2:
                    require(a == b == c == d == e == 0, 'refresh reserved words')
                    dirty = False
                elif tag == 3:
                    span(a, b)
                    require(c == d == e == 0, 'rescale reserved words')
                    predefined = dirty = False
                else:
                    span(a, b)
                    integer(c, 0, MAX_SOURCE, 'query LL length')
                    integer(d, 1, 0xffffffff, 'query encoded offset')
                    integer(e, 3, MAX_SOURCE, 'query match length')
                    require(not dirty, 'query requires refreshed prices')
                    require(b == 0 or self.literal_mode == 1 or predefined or literal_mass,
                            'positive dynamic literals require nonzero statistics')
        return self

    def packet(self):
        self.validate()
        data = MAGIC + struct.pack('<III', len(self.source), len(self.operations), self.literal_mode)
        data += self.source
        data += b''.join(struct.pack('<6I', *op) for op in self.operations)
        require(len(data) <= MAX_PACKET, 'packet extent')
        return data


def parse_packet(data):
    require(type(data) is bytes and 44 <= len(data) <= MAX_PACKET, 'packet extent/type')
    require(data[:8] == MAGIC, 'packet magic')
    source_length, count, mode = struct.unpack_from('<III', data, 8)
    integer(source_length, 0, MAX_SOURCE, 'source extent')
    integer(count, 1, MAX_OPERATIONS, 'operation count')
    require(len(data) == 20 + source_length + 24 * count, 'exact packet length')
    start = 20 + source_length
    operations = tuple(struct.unpack_from('<6I', data, start + 24 * i) for i in range(count))
    return Request(data[20:start], operations, mode).validate()
