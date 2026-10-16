#!/usr/bin/env python3
"""Independently reconstruct complete policy-tagged native frequency fixtures."""
from pathlib import Path
import argparse,json,struct,sys
sys.dont_write_bytecode=True
from oracles import ultra_frequency as model
from oracles import ultra_scalar as m
from oracles.ultra_frequency_recipes import recipes
ROOT=Path(__file__).resolve().parents[1]
PACK_SHA='a92871e38d38d30749d7d65ac710e133500ab65348191ba9579e3592f95aa11c'
PROVENANCE_SHA='4edce3cc10dd2f13551a33b39b7ebda21bee1f6976c3fe809c904dfd25ccb5fa'

def reconstruct():
    pack=bytearray(b'ZUFRQF01'+struct.pack('<I',298));layouts=[]
    for name,request,_ in recipes():
        try:request.validate()
        except ValueError:continue
        original=request.original;encoded=name.encode('ascii');start=len(pack)
        pack.extend(struct.pack('<5I',len(encoded),len(original.source),len(original.operations),original.literal_mode,request.policy));pack.extend(encoded);pack.extend(original.source);offsets=[]
        records=list(model.observations(request))[1:-1]
        for op,record in zip(original.operations,records):
            offsets.append(len(pack));words=[int(record['price_type']=='predefined')]
            for key in model.prior.KEYS:words.extend(record['frequencies'][key])
            words.extend(record['sums'][key]for key in model.prior.KEYS);words.extend(record['base_prices'][key]for key in model.prior.KEYS)
            words.extend([int(record['query']is not None),*(record['query']or[0,0,0])]);m.need(len(words)==390,'complete state width')
            pack.extend(struct.pack('<6I',*op));pack.extend(struct.pack('<390I',*words))
        layouts.append(dict(name=name,start=start,policy=request.policy,records=offsets,operations=original.operations))
    m.need(len(layouts)==298 and len(pack)==26646439,'complete reconstruction shape');return bytes(pack),layouts

def check(data,expected,root_digest=True):
    m.need(type(data)is bytes and len(data)==26646439,'exact corpus extent')
    if root_digest:m.need(m.sha(data)==PACK_SHA,'frozen native fixture')
    m.need(data[:8]==b'ZUFRQF01'and struct.unpack_from('<I',data,8)[0]==298,'magic/count')
    m.need(data==expected,'complete independent state/request reconstruction')

def apply(data,patches):
    result=bytearray(data)
    for offset,removed,hexbytes in patches:
        m.integer(offset,0,len(result),'delta offset');m.integer(removed,0,len(result)-offset,'delta deletion')
        result[offset:offset+removed]=bytes.fromhex(hexbytes)
    m.need(len(result)<=40<<20,'delta extent');return bytes(result)

def controls(data,expected,layouts,out):
    m.need(not out.exists(),'fresh controls output');out.mkdir();records=[]
    def test(name,patches):
        changed=apply(data,patches)
        try:check(changed,expected,root_digest=False)
        except ValueError as error:diagnostic=str(error)
        else:raise ValueError('changed fixture accepted '+name)
        row=dict(name=name,baseline_sha256=PACK_SHA,patches=patches,bytes=len(changed),sha256=m.sha(changed),diagnostic=diagnostic)
        (out/(name+'.json')).write_text(json.dumps(row,indent=2)+'\n')
        # Verify the lossless delta record, rather than retaining26MiB per edit.
        replay=m.load((out/(name+'.json')).read_bytes());restored=apply(data,replay['patches'])
        m.need(restored==changed and len(restored)==replay['bytes']and m.sha(restored)==replay['sha256'],'complete reconstructed control')
        records.append(row)
    def word(name,offset,value):test(name,[(offset,4,struct.pack('<I',value).hex())])
    test('short',[(11,len(data)-11,'')]);test('truncated',[(len(data)-1,1,'')]);test('trailing',[(len(data),0,'21')]);test('magic',[(0,1,'21')])
    word('zero-count',8,0);word('changed-count',8,297);word('large-count',8,299)
    first=layouts[0];word('source-limit',first['start']+4,131073);word('operation-limit',first['start']+8,129)
    word('literal-mode',first['start']+12,2);word('unknown-policy',first['start']+16,1)
    word('initial-phase',first['records'][0],2);word('reserved-word',first['records'][0]+12,1)
    query=next(offset for offset,op in zip(first['records'],first['operations'])if op[0]==4)
    word('query-zero-offset',query+16,0);word('coherent-query-price',query+24+387*4,1)
    frac=next(row for row in layouts if row['policy']==2);obs=frac['records'][0]+24
    word('wrong-valid-policy',frac['start']+16,0);word('fractional-cache',obs+382*4,0)
    test('coherent-frequency-sum-cache',[(obs+4,4,struct.pack('<I',1).hex()),(obs+378*4,4,struct.pack('<I',1).hex()),(obs+382*4,4,struct.pack('<I',512).hex())])
    dirty=next(row for row in layouts if row['name']=='uf_p2_init_single_9_m0')
    offset=next(offset for offset,op in zip(dirty['records'],dirty['operations'])if op[0]==1)
    word('dirty-cache-rewritten',offset+24+382*4,999)
    upper=next(row for row in layouts if row['name']=='uf_p2_fraction_boundary_30')
    word('fractional-upper-sum',upper['records'][2]+24+378*4,8388607)
    check(data,expected);report=dict(status='passed',optimized=not __debug__,rejected=len(records),root_digest_bypassed=True,lossless_control_deltas=True,records=records)
    (out/'summary.json').write_text(json.dumps(report,indent=2)+'\n');return len(records)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--controls-dir',type=Path);args=parser.parse_args()
    folder=ROOT/'tests/fixtures/reference-ultra-frequencies';data=(folder/'cases.bin').read_bytes();provenance=(folder/'provenance.json').read_bytes()
    m.need(m.sha(provenance)==PROVENANCE_SHA,'frozen native provenance');expected,layouts=reconstruct();check(data,expected)
    rejected=controls(data,expected,layouts,args.controls_dir)if args.controls_dir else 0
    print(json.dumps(dict(status='passed',optimized=not __debug__,cases=298,transitions=4065,queries=2848,bytes=len(data),fixture_sha256=PACK_SHA,rejected=rejected),sort_keys=True))
if __name__=='__main__':main()
