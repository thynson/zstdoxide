#!/usr/bin/env python3
"""Read-only independent reconstruction of the native-audited scalar fixture."""
from pathlib import Path
import argparse,json,struct,sys
sys.dont_write_bytecode=True
from oracles import ultra_scalar as m
from oracles.ultra_scalar_recipes import recipes
ROOT=Path(__file__).resolve().parents[1]
PACK_SHA='f5510dff52e9495a257aa78f367a25d1c0b0834ccfc05f3e4ffb099cd944c9b9'
PROVENANCE_SHA='b8f53ac2f3400e87e0f6a3a77abed9fbf4f43aacbef949e229eefe30e8b83b78'
EXPECTED=recipes()

def check(data,root_digest=True):
    m.need(type(data)is bytes and len(data)==118572,'exact fixture extent')
    if root_digest:m.need(m.sha(data)==PACK_SHA,'frozen native fixture identity')
    m.need(data[:8]==b'ZUSFIX01'and struct.unpack_from('<I',data,8)[0]==2470,'magic and count')
    for index,expected in enumerate(EXPECTED):
        row=struct.unpack_from('<10Iq',data,12+48*index);request=row[:10];observed=row[10]
        m.validate(request);m.need(request==expected,'exact original request/order')
        m.need(observed==m.result(expected),'independent scalar reconstruction')
    return 2470

def controls(data,out):
    m.need(not out.exists(),'fresh control output');out.mkdir();cases={}
    def word(name,row,word,value):
        b=bytearray(data);struct.pack_into('<I',b,12+row*48+word*4,value);cases[name]=bytes(b)
    def value(name,row,new):
        b=bytearray(data);struct.pack_into('<q',b,12+row*48+40,new);cases[name]=bytes(b)
    cases.update(short=data[:11],truncated=data[:-1],trailing=data+b'!',magic=b'!'+data[1:])
    for name,count in [('zero-count',0),('changed-count',2469),('large-count',4097)]:cases[name]=data[:8]+struct.pack('<I',count)+data[12:]
    word('mode',0,0,1);word('tag',0,1,5);word('reserved',0,3,1);word('changed-valid-request',0,2,1)
    frac=next(i for i,r in enumerate(EXPECTED)if r[:2]==(2,0));word('fraction-overflow',frac,2,8388607)
    ll=next(i for i,r in enumerate(EXPECTED)if r[1]==1);word('ll-limit',ll,2,131073)
    match=next(i for i,r in enumerate(EXPECTED)if r[1]==2);word('zero-offset',match,2,0);word('short-match',match,3,2)
    inc=next(i for i,r in enumerate(EXPECTED)if r[1]==4 and m.result(r)<0);word('increment-zero-length',inc,2,0)
    value('price-overflow',0,1<<30);value('negative-unsigned-price',0,-1);value('coherent-price',0,1);value('signed-increment',inc,0)
    records=[]
    for name,b in cases.items():
        (out/(name+'.bin')).write_bytes(b)
        try:check(b,root_digest=False)
        except ValueError as error:diagnostic=str(error)
        else:raise ValueError('forged fixture accepted '+name)
        records.append(dict(name=name,sha256=m.sha(b),bytes=len(b),diagnostic=diagnostic))
    check(data);report=dict(status='passed',optimized=not __debug__,rejected=len(records),root_digest_bypassed_for_structural_controls=True,records=records)
    (out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');return len(records)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--controls-dir',type=Path);args=parser.parse_args()
    folder=ROOT/'tests/fixtures/reference-ultra-scalars';data=(folder/'cases.bin').read_bytes();raw=(folder/'provenance.json').read_bytes()
    m.need(m.sha(raw)==PROVENANCE_SHA,'frozen native provenance')
    provenance=m.load(raw);m.need(provenance['fixture_sha256']==PACK_SHA and provenance['source_packet_sha256']==m.sha(m.packet(EXPECTED)),'input/output provenance')
    count=check(data);rejected=controls(data,args.controls_dir)if args.controls_dir else 0
    print(json.dumps(dict(status='passed',optimized=not __debug__,rows=count,fixture_sha256=PACK_SHA,forged_rejections=rejected),sort_keys=True))
if __name__=='__main__':main()
