"""Additive retained optLevel 0/2 transport; the ZOPRS03 packet stays intact."""
from dataclasses import dataclass
import struct

from .optimal_retained_frames import (
    MAX_PACKET as ORIGINAL_MAX,
    REVISION,
    Request as Original,
    parse_packet as original_packet,
    require,
)

MAGIC = b'ZURTS01\0'
MAX_PACKET = 12 + ORIGINAL_MAX


@dataclass(frozen=True)
class Request:
    policy: int
    original: Original

    def validate(self):
        require(type(self.policy) is int and self.policy in (0, 2),
                'exact retained price policy')
        require(type(self.original) is Original, 'exact original retained request')
        self.original.validate()
        return self

    def packet(self):
        self.validate()
        return MAGIC + struct.pack('<I', self.policy) + self.original.packet()


def parse_packet(data):
    require(type(data) is bytes and 192 <= len(data) <= MAX_PACKET,
            'bounded retained policy packet')
    require(data[:8] == MAGIC, 'retained policy magic')
    return Request(struct.unpack_from('<I', data, 8)[0],
                   original_packet(data[12:])).validate()
