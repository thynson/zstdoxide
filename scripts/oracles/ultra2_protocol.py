"""Bounded pass-tagged Ultra2 transport, independent of native execution."""
from dataclasses import dataclass
import json
import struct

from .ultra2_validation import SourceRequest, records
from .ultra_fresh import REVISION
from .ultra_scalar import need, load
from .optimal_price_validation import fnv

MAGIC = b'ZU2PS01\0'
HEADER = 44
MAX_PACKET = HEADER + 16384
MAX_OUTPUT = 256 << 20
MAX_LINE = 2 << 20


@dataclass(frozen=True)
class Request:
    original: SourceRequest
    seed: bool

    def validate(self):
        need(type(self.original) is SourceRequest and type(self.seed) is bool,
             'exact two-pass request types')
        self.original.validate()
        return self

    def packet(self):
        self.validate()
        r = self.original
        return MAGIC + struct.pack('<9I', len(r.source), int(self.seed), r.window_log,
                                   r.hash_log, r.chain_log, r.search_log, r.min_match,
                                   r.target, r.literal_mode) + r.source


def parse_packet(data):
    need(type(data) is bytes and HEADER + 7 <= len(data) <= MAX_PACKET,
         'bounded two-pass packet')
    need(data[:8] == MAGIC, 'two-pass magic')
    size, seed, window, hashing, chain, search, minimum, target, literal = struct.unpack_from('<9I', data, 8)
    need(seed in (0, 1), 'seeding mode')
    need(len(data) == HEADER + size, 'exact two-pass extent')
    return Request(SourceRequest(data[HEADER:], (size,), window, hashing, chain,
                                 search, minimum, target, literal), bool(seed)).validate()


def observations(request):
    request.validate()
    r = request.original
    rows, _ = records(r, request.seed)
    yield dict(record='metadata', protocol='ZU2PS01', revision=REVISION,
               source_bytes=len(r.source), source_hash=fnv(r.source), bias=2,
               parameters=[r.window_log, r.hash_log, r.chain_log, r.search_log,
                           r.min_match, r.target, r.literal_mode], seed=request.seed,
               path_cells=4099, padding_passes=2,
               state_encoding='canonical-deltas-from-zero', price_policy=2)
    for padding in range(2):
        for row in rows:
            # The Python checked-source read counter is a model diagnostic,
            # not an observation available from the unchanged C provider.
            row = {k: v for k, v in row.items() if k != 'source_reads'}
            if row['record'] == 'event':
                row = dict(row, source_unchanged=True, guards_unchanged=True, path_tail_zero=True)
            yield dict(row, padding_pass=padding)
    yield dict(record='complete', padding_passes=2,
               algorithm_passes=2 * sum(row['record'] == 'pass_end' for row in rows),
               events=2 * sum(row['record'] == 'event' for row in rows),
               source_unchanged=True, guards_unchanged=True)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False)


def validate_output(data, request):
    import io
    need(type(data) is bytes and 0 < len(data) <= MAX_OUTPUT and data.endswith(b'\n'),
         'bounded newline-terminated two-pass observations')
    stream = io.BytesIO(data)
    for expected in observations(request):
        line = stream.readline(MAX_LINE + 1)
        need(0 < len(line) <= MAX_LINE and line.endswith(b'\n'), 'bounded observation line')
        need(canonical(load(line)) == canonical(expected), 'complete independent two-pass state')
    need(stream.read(1) == b'', 'exact two-pass observation EOF')
