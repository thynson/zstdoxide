#!/usr/bin/env python3
"""Project fully audited native scalar observations, preserving original inputs."""
import json,struct,sys
from pathlib import Path
sys.dont_write_bytecode=True
import check_ultra_scalar_capture as audit
import ultra_scalar_capture_plan as plan
from oracles import ultra_scalar as m
from oracles.ultra_scalar_recipes import recipes
root=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();m.need(not out.exists(),'fresh fixture output')
qualification=audit.audit(root);rows=recipes();normal=(root/'logs/normal-run-scalar-boundaries.stdout').read_bytes();sanitized=(root/'logs/sanitized-run-scalar-boundaries.stdout').read_bytes()
m.need(normal==sanitized,'identical complete native observations');records=[m.load(line)for line in normal.splitlines()][1:-1]
m.need(len(records)==len(rows)==2470,'complete native row inventory')
pack=b'ZUSFIX01'+struct.pack('<I',len(rows))+b''.join(struct.pack('<10Iq',*row,record['value'])for row,record in zip(rows,records))
m.need(len(pack)==118572,'exact fixture bound')
provenance=dict(schema_version=1,revision=m.REVISION,rows=len(rows),fixture_bytes=len(pack),fixture_sha256=m.sha(pack),source_packet_sha256=m.sha(m.packet(rows)),source_manifest_sha256=m.sha((plan.ROOT/plan.MANIFEST).read_bytes()),capture_evidence_sha256=m.sha((root/'evidence.json').read_bytes()),normal_stdout_sha256=m.sha(normal),sanitized_stdout_sha256=m.sha(sanitized),projector_sha256=m.sha(Path(__file__).read_bytes()),capture_audit=qualification,qualification='Synthetic scalar observations from unchanged pinned C functions, fully audited before projection. No ordinary lifecycle or frame claim.')
out.mkdir(parents=True);(out/'cases.bin').write_bytes(pack);(out/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n');print(json.dumps(provenance,sort_keys=True))
