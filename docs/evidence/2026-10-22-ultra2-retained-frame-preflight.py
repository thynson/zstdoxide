"""Freeze isolated frame preflight without admitting the draft to production."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess
R=Path('/Users/lanxingcan/Workspace/zstdoxize');T=Path('/private/tmp');B=T/'retained-ultra2-frame-preflight-bundle'
def need(v,m):
 if not v:raise ValueError(m)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
names=['integration-preview-1','integration-preview-unoptimized-1','wire-preview-1','wire-preview-native-1','wire-preview-optimized-1']
stages={n:T/('retained-ultra2-'+n)for n in names};evidence={n:json.loads((p/'evidence.json').read_text())for n,p in stages.items()}
need(all(e['status']=='passed'for e in evidence.values()),'all completed isolated experiments')
base=evidence['integration-preview-unoptimized-1'];source=base['source'];need(source==evidence['integration-preview-1']['source'],'exact cross-profile source')
for label in ['integration-preview-1','integration-preview-unoptimized-1']:
 for n,h in source.items():need(sha(stages[label]/'workspace'/n)==h,'unchanged draft source '+n)
allowed={'docs/data-structures.md','src/lib.rs','src/optimal_state.rs','src/reference_search.rs','src/reference_frame_encode.rs','src/ultra2_retained_state.rs','tests/support/ultra2_retained_tokens.rs','tests/support/ultra2_retained_frame_encode.rs'}
differences={n for n,h in source.items()if not(R/n).exists()or sha(R/n)!=h};need(differences==allowed,'exact eight unpromoted integration changes')
ordinary=json.loads((stages['wire-preview-1']/'controls.stdout').read_text());optimized=json.loads((stages['wire-preview-optimized-1']/'controls.stdout').read_text());need(ordinary==optimized,'identical interpreter reports')
need(ordinary['negative_controls']==34 and ordinary['positive_controls']==2 and ordinary['child_processes']==0 and len(ordinary['baseline_audit']['cases'])==29,'complete independent controls')
for n,h in evidence['wire-preview-1']['source'].items():need(sha(R/n)==h,'unchanged independent model')
capture=stages['wire-preview-1']/'capture';hashes={p.name:sha(p)for p in capture.iterdir()};need(len(hashes)==59,'complete capture inventory')
need(hashes==base['capture']==evidence['wire-preview-optimized-1']['capture'],'same profile/interpreter captures')
for n,h in hashes.items():need(sha(stages['integration-preview-unoptimized-1']/'capture'/n)==h==evidence['wire-preview-native-1']['capture'][n]['sha256'],'same native decoded captures')
native=evidence['wire-preview-native-1'];need(len(native['commands'])==34 and all(c['exit_code']==0 for c in native['commands']),'complete native preflight')
for row in native['native']:need(sha(Path(row['path']))==row['sha256']==sha(stages['wire-preview-native-1']/row['copy']),'unchanged native inputs')
for label in ['integration-preview-1','integration-preview-unoptimized-1']:
 need('test result: ok. 13 passed; 0 failed; 0 ignored;'in(stages[label]/'test.stdout').read_text(),'thirteen profile tests')
need(not B.exists(),'fresh preflight bundle');B.mkdir();shutil.copyfile(__file__,B/'driver.py')
for n in source:
 p=B/'source'/n;p.parent.mkdir(parents=True,exist_ok=True)
 if n.startswith('tests/fixtures/'):os.link(stages['integration-preview-unoptimized-1']/'workspace'/n,p)
 else:shutil.copyfile(stages['integration-preview-unoptimized-1']/'workspace'/n,p)
for label,stage in stages.items():
 destination=B/'experiments'/label;destination.mkdir(parents=True)
 for p in stage.iterdir():
  if p.name=='workspace':continue
  if p.is_file():shutil.copyfile(p,destination/p.name)
  else:shutil.copytree(p,destination/p.name)
for name in ['retained-ultra2-tokens-draft','retained-ultra2-integration-draft','retained-ultra2-wire-draft']:shutil.copytree(T/name,B/'draft-history'/name)
for n in evidence['wire-preview-1']['source']:
 p=B/'model-source'/n;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/n,p)
commands=[]
for label,e in evidence.items():
 for c in e.get('commands',[]):commands.append(dict(experiment=label,**c))
 if label=='wire-preview-optimized-1':commands.append(dict(experiment=label,argv=e['argv'],exit_code=e['exit_code']))
need(len(commands)==40,'forty recorded preflight commands')
report=dict(status='passed',source_unchanged=True,source=source,commands=commands,unpromoted_paths=sorted(differences),experiments={n:sha(p/'evidence.json')for n,p in stages.items()},formal_execution_receipt_sha256=sha(R/'docs/evidence/2026-10-22-ultra2-retained-execution.json'),qualification='ISOLATED UNPROMOTED DRAFT PREFLIGHT ONLY. Thirteen integration tests pass in optimized and unoptimized safe builds, with all-target/all-feature draft Clippy; 59 captures match exactly across profiles. Twenty-nine frames decode through pinned zstd v1.5.7; ordinary/optimized independent source models match with 34 negative and two positive controls each. This is NOT the final 22-product matrix, no_std/platform gate, production integration or public admission. Exactly eight draft source paths differ from committed production; all other source inputs match. Identical duplicate workspace snapshots are represented once under source; all experiment receipts, logs, captures and native artifacts are preserved. Ordinary test executable contents are identified by hashes, not included.')
(B/'evidence.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
subprocess.run(['python3','-B',str(T/'compact-retained-ultra2-evidence.py'),str(B),'2026-10-22-ultra2-retained-frame-preflight'],cwd=R,check=True)
p=R/'TODO.md';s=p.read_text();anchor='- [ ] Integrate retained token collection and shared frame publication with discarded-token isolation and raw/RLE rollback.';need(s.count(anchor)==1,'pending formal integration');p.write_text(s.replace(anchor,'- [x] Preserve isolated private-frame preflight with cross-profile captures, independent actual-wire replay and pinned native decoding.\n'+anchor))
a=json.loads((R/'docs/evidence/2026-10-22-ultra2-retained-frame-preflight.json').read_text())
p=R/'docs/reference-ultra2-retained-implementation.md';p.write_text(p.read_text()+f'''\n## Isolated private-frame preflight (not production admission)\n\nThe token/frame integration remains an external draft. Its 13 tests pass in\noptimized and unoptimized safe builds, and strict all-target/all-feature draft\nClippy passes. All 59 capture artifacts match across profiles. The 29 complete\nframes decode exactly through pinned upstream v1.5.7. Independent ordinary and\noptimized Python reports match: 116881 events, 672 sequences, 34 rejected\nforgeries and two positive audits per mode, with child execution forbidden.\nThe seven-byte first-block case uses actual kinds [2,1] rather than the original\nnative hypothesis [0,1]; actual-kind model replay and native decoding both pass.\n\nThe preflight archive preserves all 40 command records, logs, captures, native\nartifacts and exact draft sources, with precisely eight unpromoted paths listed.\nIt binds every other draft input to committed production. Formal integration,\nits 22-product/21-no_std matrix, foreign platform gates and public admission\nremain unchecked. Evidence: `docs/evidence/2026-10-22-ultra2-retained-frame-preflight.{{json,tar.gz}}`,\nSHA-256 `{a['archive_sha256']}`.\n''')
print('isolated preflight preserved without production admission')
