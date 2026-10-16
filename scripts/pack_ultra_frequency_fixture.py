#!/usr/bin/env python3
"""Project complete audited native frequency states without changing requests."""
from pathlib import Path
import json,struct,sys
sys.dont_write_bytecode=True
import check_ultra_frequency_capture as audit
import ultra_frequency_capture_plan as plan
from oracles import ultra_scalar as scalar
from oracles import ultra_frequency as model
from oracles.ultra_frequency_recipes import recipes
root=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();scalar.need(not out.exists(),'fresh projection')
qualification=audit.audit(root);pack=bytearray(b'ZUFRQF01'+struct.pack('<I',298));catalog=[];transitions=queries=0
for name,request,previous in recipes():
    try:request.validate()
    except ValueError:continue
    original=request.original;packet=(root/'requests'/(name+'.bin')).read_bytes();scalar.need(packet==request.packet(),'unchanged original request')
    logs=[(root/'logs'/(mode+'-run-'+name+'.stdout')).read_bytes()for mode in ['normal','sanitized']];scalar.need(logs[0]==logs[1],'complete native equality')
    rows=[scalar.load(line)for line in logs[0].splitlines()][1:-1];scalar.need(len(rows)==len(original.operations),'complete operation inventory')
    start=len(pack);encoded=name.encode('ascii');pack.extend(struct.pack('<5I',len(encoded),len(original.source),len(original.operations),original.literal_mode,request.policy));pack.extend(encoded);pack.extend(original.source)
    for op,row in zip(original.operations,rows):
        scalar.need(row['price_policy']==request.policy,'observed explicit policy');words=[int(row['price_type']=='predefined')]
        for key in model.prior.KEYS:words.extend(row['frequencies'][key])
        words.extend(row['sums'][key]for key in model.prior.KEYS);words.extend(row['base_prices'][key]for key in model.prior.KEYS);words.extend([int(row['query']is not None),*(row['query']or[0,0,0])]);scalar.need(len(words)==390,'complete fixed state record')
        pack.extend(struct.pack('<6I',*op));pack.extend(struct.pack('<390I',*words));transitions+=1;queries+=int(row['query']is not None)
    catalog.append(dict(id=name,original_id=previous,policy=request.policy,offset=start,bytes=len(pack)-start,operations=len(rows),request_sha256=scalar.sha(packet),transcript_sha256=scalar.sha(logs[0]),source_sha256=scalar.sha(original.source)))
scalar.need(len(catalog)==298 and transitions==4065 and queries==2848 and len(pack)<=40<<20,'complete bounded projection')
provenance=dict(schema_version=1,revision=model.REVISION,cases=298,transitions=transitions,queries=queries,fixture_bytes=len(pack),fixture_sha256=scalar.sha(pack),capture_evidence_sha256=scalar.sha((root/'evidence.json').read_bytes()),source_manifest_sha256=scalar.sha((plan.ROOT/plan.MANIFEST).read_bytes()),projector_sha256=scalar.sha(Path(__file__).read_bytes()),capture_audit=qualification,catalog=catalog,qualification='Complete audited native policy-tagged states; original whole-mode requests unchanged. No production Rust or Ultra path/frame admission.')
out.mkdir(parents=True);(out/'cases.bin').write_bytes(pack);(out/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n');print(json.dumps({k:v for k,v in provenance.items()if k!='catalog'},sort_keys=True))
