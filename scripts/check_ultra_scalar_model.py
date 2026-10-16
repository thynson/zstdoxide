#!/usr/bin/env python3
"""Exercise independent scalar model and transport without native execution."""
import json
from pathlib import Path
import struct
import sys
sys.dont_write_bytecode = True
from oracles import ultra_scalar as m
from oracles.ultra_scalar_recipes import recipes, manifest

def rejects(fn):
    try: fn()
    except (ValueError, TypeError): return
    raise ValueError('expected rejection')

rows=recipes();packet=m.packet(rows)
m.need(m.parse(packet)==rows,'exact packet roundtrip')
m.need(len(rows)==len(set(rows)) and len(rows)<=4096,'unique bounded inventory')
m.need(set((r[0],r[1])for r in rows)=={(mode,tag)for mode in (0,2)for tag in range(5)},'both modes and all operations')
for value,expected in [(0,256),(1,512),(2,640),(3,768),(6,960),(7,1024),(8388606,6143)]:
    m.need(m.weight(2,value)==expected,'manually derived interpolation')
for mode in (0,2):
    for exponent in range(23):
        raw=2**exponent-1
        m.need(m.weight(mode,raw)==(exponent+(mode==2))*256,'exact power')
        if raw:
            m.need(m.weight(mode,raw-1)<=m.weight(mode,raw),'power monotonicity')
for row in rows:
    result=m.result(row)
    if row[1]==0 and row[2]<=m.FRACTION_MAX:
        m.need(256<=m.weight(2,row[2])-m.weight(0,row[2])<=511,'fractional intercept domain')
    if row[1]==3 and row[2]:m.need(result>=row[2]*256,'literal clamp')
m.need(m.result((2,4,1,17,1,15,0,0,0,0))<0,'negative LL increment')
m.need(m.result((2,4,131072,17,3,3,0,0,0,0))==256,'full-block increment')
for invalid in [-1,8388607,0xffffffff,True,1.0]:rejects(lambda invalid=invalid:m.weight(2,invalid))
for mode in [1,3,True,0.0]:rejects(lambda mode=mode:m.weight(mode,0))
for bad in [packet[:11],packet[:-1],packet+b'!',b'!'+packet[1:],packet[:8]+struct.pack('<I',0)+packet[12:],packet[:8]+struct.pack('<I',4097)+packet[12:]]:rejects(lambda bad=bad:m.parse(bad))
for word,value in [(0,1),(1,5),(2,0xffffffff),(3,1)]:
    data=bytearray(m.packet(((2,0,0,0,0,0,0,0,0,0),)));struct.pack_into('<I',data,12+word*4,value);rejects(lambda data=data:m.parse(bytes(data)))
records=list(m.records(rows));transcript=('\n'.join(json.dumps(r,sort_keys=True)for r in records)+'\n').encode();m.validate_output(transcript,rows)
for bad in [transcript+b'{}\n',transcript.replace(b'"schema_version": 1',b'"schema_version": true',1),transcript.replace(b'"schema_version": 1',b'"schema_version": 1, "schema_version": 1',1),transcript.replace(b'"value": 0',b'"value": NaN',1)]:rejects(lambda bad=bad:m.validate_output(bad,rows))
path=Path('tests/fixtures/reference-ultra-scalar-recipes/recipes.json')
m.need(path.read_text()==json.dumps(manifest(),sort_keys=True,indent=2)+'\n','frozen source-only recipes')
counts={str((mode,tag)):sum(r[:2]==(mode,tag)for r in rows)for mode in (0,2)for tag in range(5)}
print(json.dumps(dict(status='passed',optimized=not __debug__,rows=len(rows),packet_bytes=len(packet),packet_sha256=m.sha(packet),negative_increments=sum(r[1]==4 and m.result(r)<0 for r in rows),counts=counts),sort_keys=True))
