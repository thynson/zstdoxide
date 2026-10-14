"""Bounded source-only fresh ordinary Optimal requests; no codec invocation."""
from dataclasses import dataclass
import struct
from .optimal_matches_broad import REVISION, require, integer

MAGIC = b'ZOPST01\0'
MAX_SOURCE = 512
MAX_PACKET = 40 + MAX_SOURCE

@dataclass(frozen=True)
class Request:
    source: bytes
    window_log: int = 10
    hash_log: int = 6
    chain_log: int = 6
    search_log: int = 1
    min_match: int = 4
    target: int = 0
    literal_mode: int = 0

    @property
    def hash3_log(self):
        return min(self.window_log,17) if self.min_match == 3 else 0

    def validate(self):
        require(type(self.source) is bytes, 'immutable source bytes')
        integer(len(self.source),64,MAX_SOURCE,'bounded fresh block')
        for value,low,high,name in [(self.window_log,10,12,'window'),(self.hash_log,6,9,'hash'),
                (self.chain_log,6,9,'chain'),(self.search_log,1,4,'search'),
                (self.min_match,3,7,'minimum'),(self.target,0,4095,'target'),(self.literal_mode,0,1,'literal mode')]:
            integer(value,low,high,name)
        return self

    def packet(self):
        self.validate()
        return MAGIC+struct.pack('<8I',len(self.source),self.window_log,self.hash_log,self.chain_log,
            self.search_log,self.min_match,self.target,self.literal_mode)+self.source


def parse_packet(data):
    require(type(data) is bytes and 104<=len(data)<=MAX_PACKET,'bounded immutable packet')
    require(data[:8]==MAGIC,'startup magic')
    n,w,h,c,s,m,t,l=struct.unpack_from('<8I',data,8)
    require(len(data)==40+n,'exact source extent')
    return Request(data[40:],w,h,c,s,m,t,l).validate()
