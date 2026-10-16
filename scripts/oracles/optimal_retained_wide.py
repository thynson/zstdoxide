"""Wider sparse ordinary retained Optimal transport; no native codec invocation."""
from dataclasses import dataclass
import struct
from .optimal_startup import REVISION,require
from .optimal_matches_broad import integer

MAGIC=b'ZOPRW02\0'
MAX_SOURCE=16384
MAX_BLOCK=8192
MAX_EVENTS=32768
MAX_OUTPUT=256<<20
MAX_PACKET=44+16+MAX_SOURCE

@dataclass(frozen=True)
class Request:
    source: bytes
    blocks: tuple
    window_log: int=10
    hash_log: int=6
    chain_log: int=6
    search_log: int=1
    min_match: int=4
    target: int=0
    literal_mode: int=0

    @property
    def hash3_log(self):return self.window_log if self.min_match==3 else 0
    @property
    def minimum(self):return 3 if self.min_match==3 else 4

    def validate(self):
        require(type(self.source)is bytes and type(self.blocks)is tuple,'owned immutable source/schedule')
        integer(len(self.source),128,MAX_SOURCE,'retained source extent')
        integer(len(self.blocks),2,4,'retained block count')
        for n in self.blocks:integer(n,64,MAX_BLOCK,'bounded block')
        require(sum(self.blocks)==len(self.source),'exact complete schedule')
        for value,lo,hi,name in [(self.window_log,10,16,'window'),(self.hash_log,6,12,'hash'),
            (self.chain_log,6,12,'chain'),(self.search_log,1,6,'search'),(self.min_match,3,7,'minimum'),
            (self.target,0,4095,'target'),(self.literal_mode,0,1,'literal mode')]:integer(value,lo,hi,name)
        require(len(self.source)<=1<<self.window_log,'complete source inside one window')
        return self

    def packet(self):
        self.validate()
        values=(len(self.source),len(self.blocks),self.window_log,self.hash_log,self.chain_log,
                self.search_log,self.min_match,self.target,self.literal_mode,*self.blocks)
        return MAGIC+struct.pack('<'+'I'*len(values),*values)+self.source


def parse_packet(data):
    require(type(data)is bytes and 180<=len(data)<=MAX_PACKET,'bounded retained packet')
    require(data[:8]==MAGIC,'retained magic')
    n,count,w,h,c,s,m,t,l=struct.unpack_from('<9I',data,8)
    integer(count,2,4,'bounded schedule before unpack')
    require(len(data)==44+4*count+n,'exact retained packet extent')
    blocks=struct.unpack_from('<'+'I'*count,data,44)
    return Request(data[44+4*count:],blocks,w,h,c,s,m,t,l).validate()
