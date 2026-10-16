#!/usr/bin/env python3
"""Reconcile the successor model with all original immutable native states."""
from pathlib import Path
import json,struct,sys
sys.dont_write_bytecode=True
from oracles import ultra_frequency as m
from oracles.ultra_frequency_recipes import recipes,manifest
from oracles import ultra_scalar as scalar

pack=bytearray(b'ZOPFX01\0'+struct.pack('<I',147));transitions=queries=accepted=rejected=0;boundary=[]
for name,request,previous in recipes():
    try:request.validate()
    except ValueError:
        rejected+=1
        try:m.parse(request.raw_packet())
        except ValueError:continue
        raise ValueError('negative wrapper parsed')
    accepted+=1;m.need(m.parse(request.packet())==request,'successor roundtrip')
    records=list(m.observations(request));operations=records[1:-1];transitions+=len(operations);queries+=sum(r['query']is not None for r in operations)
    if previous is None:boundary.append(dict(id=name,largest_literal_sum=max(r['sums']['lit']for r in operations),final_sum=operations[-1]['sums']['lit']))
    if request.policy==0 and previous is not None:
        original=request.original;encoded=previous.encode('ascii');pack.extend(struct.pack('<4I',len(encoded),len(original.source),len(original.operations),original.literal_mode));pack.extend(encoded);pack.extend(original.source)
        for op,r in zip(original.operations,operations):
            words=[int(r['price_type']=='predefined')]
            for key in m.prior.KEYS:words.extend(r['frequencies'][key])
            words.extend(r['sums'][key]for key in m.prior.KEYS);words.extend(r['base_prices'][key]for key in m.prior.KEYS)
            words.extend([int(r['query']is not None),*(r['query']or[0,0,0])]);m.need(len(words)==390,'fixed state record')
            pack.extend(struct.pack('<6I',*op));pack.extend(struct.pack('<390I',*words))
original=Path('tests/fixtures/reference-optimal-prices/cases.bin').read_bytes()
m.need(len(original)==13036992 and scalar.sha(original)=='c15e68d7c570b262cd3efb2e0be021cca3d0fc83afe6c26eb3527805b67978a4','immutable original C fixture')
m.need(bytes(pack)==original,'every original native state/query reconstructed')
m.need(accepted==298 and rejected==2 and transitions==4065 and queries==2848,'complete successor inventory')
m.need(any(r['largest_literal_sum']==8388606 for r in boundary),'exact final fractional mass')
p=Path('tests/fixtures/reference-ultra-frequency-recipes/recipes.json');m.need(p.read_text()==json.dumps(manifest(),sort_keys=True,indent=2)+'\n','frozen successor manifest')
print(json.dumps(dict(status='passed',optimized=not __debug__,accepted=accepted,rejected=rejected,transitions=transitions,queries=queries,original_fixture_sha256=scalar.sha(original),boundaries=boundary),sort_keys=True))
