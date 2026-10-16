"""Additive policy-tagged no-dictionary frequency protocol and independent model.

Original whole-bit requests/model stay immutable. Fractional arithmetic comes
from the independently native-qualified scalar model, never production Rust.
"""
from dataclasses import dataclass
import bisect,struct
from . import optimal_prices_v2 as old
from . import optimal_price_validation as prior
from . import ultra_scalar as scalar

REVISION=old.REVISION
MAGIC=b'ZUFREQ01'
MAX_PACKET=old.MAX_PACKET+4
need=old.require

@dataclass(frozen=True)
class Request:
    policy:int
    original:old.Request

    def validate(self):
        need(type(self.policy)is int and self.policy in (0,2),'coupled price policy')
        need(type(self.original)is old.Request,'original bounded request')
        self.original.validate()
        if self.policy==2 and self.original.literal_mode==0:
            _,offset,length,*_=self.original.operations[0]
            histogram=[0]*256
            for byte in self.original.source[offset:offset+length]:histogram[byte]+=1
            initial=sum(int(n>0)+(n//256)for n in histogram)
            updates=sum(op[2]*(op[5]if op[0]==5 else 1)for op in self.original.operations if op[0]in (1,5))
            need(initial+2*updates<=scalar.FRACTION_MAX,'conservative fractional literal mass')
        return self

    def raw_packet(self):
        original=self.original.packet()
        return MAGIC+original[8:20]+struct.pack('<I',self.policy)+original[20:]

    def packet(self):
        self.validate();return self.raw_packet()


def parse(data):
    need(type(data)is bytes and 48<=len(data)<=MAX_PACKET,'frequency packet bounds')
    need(data[:8]==MAGIC,'frequency magic')
    policy=struct.unpack_from('<I',data,20)[0]
    original=old.parse_packet(old.MAGIC+data[8:20]+data[24:])
    return Request(policy,original).validate()


class Model(prior.Model):
    def __init__(self,policy,literal_mode):
        scalar.maximum(policy);super().__init__(literal_mode);self.policy=policy

    def refresh(self):
        for key in prior.KEYS:
            if key!='lit'or self.mode==0:self.cache[key]=scalar.weight(self.policy,self.sums[key])

    def literal_price(self,literals):
        if not literals:return 0
        if self.mode==1:return prior.finite(len(literals)*2048)
        if self.predefined:return prior.finite(len(literals)*1536)
        base=self.cache['lit'];need(base>=256,'nonzero literal cache')
        return prior.finite(sum(max(256,base-scalar.weight(self.policy,self.tables['lit'][byte]))for byte in literals))

    def ll_price(self,length):
        if self.predefined:return scalar.weight(self.policy,length)
        if length==131072:return prior.finite(256+self.ll_price(length-1))
        code=bisect.bisect_right(prior.LL_START,length)-1
        return prior.finite(prior.LL_WIDTHS[code]*256+self.cache['ll']-scalar.weight(self.policy,self.tables['ll'][code]))

    def match_price(self,offset,length):
        code=offset.bit_length()-1
        if self.predefined:return prior.finite(scalar.weight(self.policy,length-3)+(16+code)*256)
        ml=bisect.bisect_right(prior.ML_START,length)-1
        penalty=max(0,code-19)*512 if self.policy==0 else 0
        return prior.finite(code*256+self.cache['of']-scalar.weight(self.policy,self.tables['of'][code])+penalty+
                            prior.ML_WIDTHS[ml]*256+self.cache['ml']-scalar.weight(self.policy,self.tables['ml'][ml])+51)

    def update(self,literals,offset,length):
        # Batched histogram arithmetic independently reproduces individual C
        # increments; a transcript is still emitted after each scheduled op.
        if self.mode==0:
            histogram=[0]*256
            for byte in literals:histogram[byte]+=1
            for byte,count in enumerate(histogram):self.tables['lit'][byte]=prior.integer(self.tables['lit'][byte]+2*count,0,prior.U32_MAX-1,'literal increment')
            self.sums['lit']=prior.integer(self.sums['lit']+2*len(literals),0,prior.U32_MAX-1,'literal sum')
        for key,code in [('ll',bisect.bisect_right(prior.LL_START,len(literals))-1),('ml',bisect.bisect_right(prior.ML_START,length)-1),('of',offset.bit_length()-1)]:
            self.tables[key][code]=prior.integer(self.tables[key][code]+1,0,prior.U32_MAX-1,'symbol increment')
            self.sums[key]=prior.integer(self.sums[key]+1,0,prior.U32_MAX-1,'symbol sum')
        need(all(total<=scalar.maximum(self.policy)for total in self.sums.values()),'all policy counter bounds')


def observations(request):
    request.validate();original=request.original;state=Model(request.policy,original.literal_mode);identity=prior.fnv(original.source)
    yield dict(record='request',schema_version=1,revision=REVISION,source_bytes=len(original.source),source_fnv1a64=identity,operations=len(original.operations),literal_mode=original.literal_mode,price_policy=request.policy)
    for index,(tag,a,b,c,d,e)in enumerate(original.operations):
        query=None
        if tag in (0,3):state.rescale(original.source[a:a+b])
        elif tag in (1,5):
            for _ in range(e if tag==5 else 1):state.update(original.source[a:a+b],c,d)
        elif tag==2:state.refresh()
        else:query=[state.literal_price(original.source[a:a+b]),state.ll_price(c),state.match_price(d,e)]
        record=state.record(index,tag,query);record['price_policy']=request.policy;yield record
    yield dict(record='end',operations=len(original.operations),source_fnv1a64=identity,price_policy=request.policy)
