"""Broader source-only sorted-tree/all-match requests; no codec invocation."""
from dataclasses import dataclass
import struct

REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAGIC = b'ZOPMB02\0'
MAX_SOURCE = 524288
MAX_QUERIES = 16
MAX_PACKET = 524716


def require(value, reason):
    if not value:
        raise ValueError(reason)


def integer(value, low, high, reason):
    require(type(value) is int and low <= value <= high, reason)
    return value


@dataclass(frozen=True)
class Request:
    source: bytes
    queries: tuple[tuple[int, int, int, int, int, int], ...]
    window_log: int = 10
    hash_log: int = 6
    chain_log: int = 6
    search_log: int = 1
    min_match: int = 4
    target: int = 0
    hash3_log: int = 0

    def validate(self):
        require(type(self.source) is bytes, 'owned immutable source')
        integer(len(self.source), 8, MAX_SOURCE, 'source extent')
        require(type(self.queries) is tuple, 'immutable query schedule')
        integer(len(self.queries), 1, MAX_QUERIES, 'query count')
        for value, low, high, name in [(self.window_log,10,21,'window log'),
                (self.hash_log,6,12,'hash log'), (self.chain_log,6,12,'chain log'),
                (self.search_log,1,8,'search log'), (self.min_match,3,7,'minimum match'),
                (self.target,0,131072,'applied target')]:
            integer(value, low, high, name)
        if self.min_match == 3:
            integer(self.hash3_log, min(self.window_log, 17), min(self.window_log, 17), 'ordinary short-key table log')
        else:
            integer(self.hash3_log, 0, 0, 'absent short-key table')
        previous = -1
        for query in self.queries:
            require(type(query) is tuple and len(query) == 6, 'six immutable query words')
            position, rep0, rep1, rep2, ll0, beat = query
            integer(position, 0, len(self.source) - 8, 'query footprint')
            require(position > previous, 'strictly increasing queries')
            previous = position
            for repeat in (rep0, rep1, rep2):
                integer(repeat, 0, min(4096, position + 2), 'bounded repeat pointer formation')
            integer(ll0, 0, 1, 'literal-zero flag')
            integer(beat, 3 if self.min_match == 3 else 4,
                    min(4096, len(self.source) - position), 'length to beat')
        return self

    def packet(self):
        self.validate()
        fields = (len(self.source), len(self.queries), self.window_log, self.hash_log,
                  self.chain_log, self.search_log, self.min_match, self.target, self.hash3_log)
        return MAGIC + struct.pack('<9I', *fields) + self.source + b''.join(struct.pack('<6I', *q) for q in self.queries)


def parse_packet(data):
    require(type(data) is bytes and 76 <= len(data) <= MAX_PACKET, 'packet extent/type')
    require(data[:8] == MAGIC, 'packet magic')
    n, count, w, h, c, s, m, target, h3 = struct.unpack_from('<9I', data, 8)
    integer(n, 8, MAX_SOURCE, 'source extent')
    integer(count, 1, MAX_QUERIES, 'query count')
    require(len(data) == 44 + n + 24 * count, 'exact packet extent')
    start = 44 + n
    queries = tuple(struct.unpack_from('<6I', data, start + 24 * i) for i in range(count))
    return Request(data[44:start], queries, w, h, c, s, m, target, h3).validate()
