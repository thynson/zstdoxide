"""Bounded phase/block transport for independent retained Ultra2 predictions.

One-block packets are private predecessor controls, never retained public
admission. Bootstrap-disabled policy 0/2 controls preserve prior models.
"""
from dataclasses import dataclass
import struct

from .ultra2_validation import SourceRequest as FreshRequest
from .ultra2_lifecycle import integer
from .ultra_scalar import need
from .ultra_fresh import REVISION

MAGIC = b'ZU2RT01\0'
HEADER = 52
MAX_PACKET = HEADER + 16 + 32768
MAX_OUTPUT = 256 << 20
MAX_LINE = 2 << 20


@dataclass(frozen=True)
class SourceRequest(FreshRequest):
    def validate(self):
        need(type(self.source) is bytes and type(self.blocks) is tuple,
             'immutable complete source and copied schedule')
        integer(len(self.source), 7, 32768, 'bounded complete source')
        integer(len(self.blocks), 1, 4, 'private block count')
        for length in self.blocks:
            integer(length, 7, 16384 if len(self.blocks) == 1 else 8192,
                    'bounded complete block')
        need(sum(self.blocks) == len(self.source), 'exact schedule extent')
        for value, lo, hi, name in (
            (self.window_log, 10, 16, 'window'), (self.hash_log, 6, 12, 'hash'),
            (self.chain_log, 6, 12, 'chain'), (self.search_log, 1, 6, 'search'),
            (self.min_match, 3, 7, 'minimum'), (self.target, 0, 4095, 'target'),
            (self.literal_mode, 0, 1, 'literal mode')):
            integer(value, lo, hi, name)
        need(len(self.source) <= 1 << self.window_log, 'source fits one window')
        return self


@dataclass(frozen=True)
class Request:
    original: SourceRequest
    bootstrap: bool = True
    policy: int = 2

    def validate(self):
        need(type(self.original) is SourceRequest, 'exact retained source request')
        self.original.validate()
        need(type(self.bootstrap) is bool, 'exact bootstrap hypothesis')
        need(type(self.policy) is int and self.policy in (0, 2), 'price policy')
        need(not self.bootstrap or self.policy == 2, 'bootstrap requires policy 2')
        return self

    def packet(self):
        self.validate()
        r = self.original
        words = (len(r.source), len(r.blocks), int(self.bootstrap), self.policy,
                 r.window_log, r.hash_log, r.chain_log, r.search_log,
                 r.min_match, r.target, r.literal_mode, *r.blocks)
        return MAGIC + struct.pack('<' + 'I' * len(words), *words) + r.source


def parse_packet(data):
    need(type(data) is bytes and HEADER + 4 + 7 <= len(data) <= MAX_PACKET,
         'bounded immutable phase/block packet')
    need(data[:8] == MAGIC, 'retained Ultra2 magic')
    size, count, seed, policy, w, h, c, s, m, t, lit = struct.unpack_from('<11I', data, 8)
    integer(count, 1, 4, 'bounded schedule before unpack')
    need(seed in (0, 1), 'bootstrap encoding')
    need(len(data) == HEADER + 4 * count + size, 'exact packet extent')
    blocks = struct.unpack_from('<' + 'I' * count, data, HEADER)
    return Request(SourceRequest(data[HEADER + 4 * count:], blocks, w, h, c, s,
                                 m, t, lit), bool(seed), policy).validate()
