"""Bounded policy-tagged complete fresh requests; no native execution."""
from dataclasses import dataclass
import struct
from .optimal_fresh_wide import Request as Original,parse_packet as original_packet,MAX_PACKET as ORIGINAL_MAX,REVISION,require
MAGIC=b'ZUFRS01\0'
MAX_PACKET=12+ORIGINAL_MAX

@dataclass(frozen=True)
class Request:
    policy:int
    original:Original
    def validate(self):
        require(type(self.policy)is int and self.policy in(0,2),'coupled policy')
        require(type(self.original)is Original,'exact original fresh request')
        self.original.validate()
        return self
    def packet(self):
        self.validate()
        return MAGIC+struct.pack('<I',self.policy)+self.original.packet()

def parse_packet(data):
    require(type(data)is bytes and 124<=len(data)<=MAX_PACKET,'bounded fresh policy packet')
    require(data[:8]==MAGIC,'fresh policy magic')
    return Request(struct.unpack_from('<I',data,8)[0],original_packet(data[12:])).validate()
