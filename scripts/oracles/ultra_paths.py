"""Policy-tagged bounded scripted path transport; source-only and codec-free."""
from dataclasses import dataclass
import struct
from . import optimal_paths_wide as prior
from .ultra_scalar import integer,need
REVISION=prior.REVISION
MAGIC=b'ZUPTH01\0'
MAX_PACKET=prior.MAX_PACKET+4
MAX_EVENTS=prior.MAX_EVENTS
MAX_OUTPUT=prior.MAX_OUTPUT

@dataclass(frozen=True)
class Request:
    policy:int
    original:prior.Request
    def validate(self):
        need(type(self.original)is prior.Request,'exact original request')
        self.original.validate();need(type(self.policy)is int and self.policy in (0,2),'coupled path policy');return self
    def packet(self):
        self.validate();packet=self.original.packet()
        return MAGIC+packet[8:28]+struct.pack('<I',self.policy)+packet[28:]

def parse_packet(data):
    need(type(data)is bytes and 41<=len(data)<=MAX_PACKET,'bounded path packet')
    need(data[:8]==MAGIC,'path packet magic');policy=struct.unpack_from('<I',data,28)[0]
    return Request(policy,prior.parse_packet(prior.MAGIC+data[8:28]+data[32:])).validate()
