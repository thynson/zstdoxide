"""Wider bounded scripted-match Optimal path requests; no codec execution.

This protocol isolates path decisions, not ordinary match discovery. Repeat
candidates require dynamic validation against the history at the actual callback.
"""
from dataclasses import dataclass
import struct

REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAGIC = b'ZOPHW02\0'
MAX_SOURCE = 8192
MAX_POSITIONS = 64
MAX_CANDIDATES = 8
MAX_EVENTS = 16384
MAX_OUTPUT = 128 << 20
MAX_PACKET = 28 + MAX_SOURCE + MAX_POSITIONS * (8 + MAX_CANDIDATES * 8)


def require(value, reason):
    if not value:
        raise ValueError(reason)


def integer(value, low, high, reason):
    require(type(value) is int and low <= value <= high, reason)
    return value


def distance(off_base, repeats, zero_literals):
    integer(off_base, 1, 0xffffffff, 'offset intent')
    require(type(repeats) is tuple and len(repeats) == 3, 'repeat record')
    for value in repeats:
        integer(value, 1, 0xffffffff, 'positive repeat distance')
    require(type(zero_literals) is bool, 'literal-zero flag')
    if off_base > 3:
        return off_base - 3
    if zero_literals:
        return (repeats[1], repeats[2], repeats[0] - 1)[off_base - 1]
    return repeats[off_base - 1]


def next_repeats(off_base, repeats, zero_literals):
    d = distance(off_base, repeats, zero_literals)
    require(d > 0, 'nonzero resolved repeat')
    if off_base > 3:
        return (d, repeats[0], repeats[1])
    index = off_base - 1 + int(zero_literals)
    if index == 0:
        return repeats
    if index == 1:
        return (d, repeats[0], repeats[2])
    return (d, repeats[0], repeats[1])


def valid_match(source, position, off_base, length, repeats, zero_literals):
    d = distance(off_base, repeats, zero_literals)
    require(0 < d <= position and position + length <= len(source), 'available source match')
    require(source[position:position + length] == source[position - d:position - d + length], 'exact source match bytes')


@dataclass(frozen=True)
class Request:
    source: bytes
    # Each position maps to immutable (offset intent, length) pairs.
    scripts: tuple[tuple[int, tuple[tuple[int, int], ...]], ...]
    min_match: int = 3
    target: int = 64
    literal_mode: int = 0

    @property
    def minimum(self):
        return 3 if self.min_match == 3 else 4

    def validate(self):
        require(type(self.source) is bytes, 'owned immutable source')
        integer(len(self.source), 9, MAX_SOURCE, 'source extent')
        integer(self.min_match, 3, 7, 'minimum parameter')
        integer(self.target, 0, 4095, 'bounded target')
        integer(self.literal_mode, 0, 1, 'literal mode')
        require(type(self.scripts) is tuple, 'immutable script')
        integer(len(self.scripts), 0, MAX_POSITIONS, 'script count')
        previous = 0
        for entry in self.scripts:
            require(type(entry) is tuple and len(entry) == 2, 'script entry')
            position, candidates = entry
            integer(position, 1, len(self.source) - 8, 'query footprint')
            require(position > previous, 'strict script positions')
            previous = position
            require(type(candidates) is tuple, 'immutable candidates')
            integer(len(candidates), 1, MAX_CANDIDATES, 'candidate count')
            prior_length = self.minimum - 1
            for candidate in candidates:
                require(type(candidate) is tuple and len(candidate) == 2, 'candidate pair')
                off_base, length = candidate
                integer(off_base, 1, position + 3, 'bounded intent')
                integer(length, self.minimum, len(self.source) - position, 'candidate extent')
                require(length > prior_length, 'strict candidate lengths')
                prior_length = length
                if off_base > 3:
                    valid_match(self.source, position, off_base, length, (1, 4, 8), False)
        return self

    def packet(self):
        self.validate()
        result = MAGIC + struct.pack('<5I', len(self.source), len(self.scripts), self.min_match, self.target, self.literal_mode) + self.source
        for position, candidates in self.scripts:
            result += struct.pack('<2I', position, len(candidates))
            result += b''.join(struct.pack('<2I', *pair) for pair in candidates)
        require(len(result) <= MAX_PACKET, 'bounded packet')
        return result


def parse_packet(data):
    require(type(data) is bytes and 37 <= len(data) <= MAX_PACKET, 'packet extent/type')
    require(data[:8] == MAGIC, 'packet magic')
    size, count, minimum, target, mode = struct.unpack_from('<5I', data, 8)
    integer(size, 9, MAX_SOURCE, 'source extent')
    integer(count, 0, MAX_POSITIONS, 'script count')
    offset = 28 + size
    require(offset <= len(data), 'source extent')
    source = data[28:offset]
    entries = []
    for _ in range(count):
        require(offset + 8 <= len(data), 'script header extent')
        position, n = struct.unpack_from('<2I', data, offset)
        offset += 8
        integer(n, 1, MAX_CANDIDATES, 'candidate count')
        require(offset + 8 * n <= len(data), 'candidate bytes')
        candidates = tuple(struct.unpack_from('<2I', data, offset + 8 * i) for i in range(n))
        offset += 8 * n
        entries.append((position, candidates))
    require(offset == len(data), 'no trailing bytes')
    return Request(source, tuple(entries), minimum, target, mode).validate()
