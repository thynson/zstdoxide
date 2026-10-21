"""Reconcile bounded retained Optimal gates without relabeling historical runs."""
from pathlib import Path
import datetime,hashlib,json
R=Path('/Users/lanxingcan/Workspace/zstdoxize');T=Path('/private/tmp');DEST=R/'docs/evidence/2026-10-21-optimal-retained-closure.json'
def need(v,m):
 if not v:raise ValueError(m)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb')as f:
  for b in iter(lambda:f.read(1<<20),b''):h.update(b)
 return h.hexdigest()
def archive_record(p):
 e=json.loads(p.read_text());a=e.get('archive')
 if a is None:return None
 if type(a)is dict:
  path=R/a['path'];digest=a['sha256'];size=a['bytes']
 else:path=p.parent/a;digest=e['archive_sha256'];size=e['archive_bytes']
 need(path.parent==R/'docs/evidence'and path.is_file()and not path.is_symlink(),'local regular archive')
 need(sha(path)==digest and path.stat().st_size==size,'complete archive identity '+str(path))
 return dict(report=str(p.relative_to(R)),report_sha256=sha(p),archive=str(path.relative_to(R)),archive_sha256=digest,archive_bytes=size)
need(not DEST.exists(),'fresh closure')
old=json.loads((R/'docs/evidence/2026-10-20-ultra2-closure.json').read_text());previous=old['verified_predecessors']+old['verified_evidence']
need(len(previous)==68 and len({r['report']for r in previous})==68,'complete prior Ultra/Ultra2 archive inventory')
for row in previous:need(sha(R/row['report'])==row['report_sha256']and sha(R/row['archive'])==row['archive_sha256'],'unchanged earlier archive/report pair')
retained_previous=[];direct=[]
for p in sorted((R/'docs/evidence').glob('2026-10-16-optimal-retained*.json')):
 row=archive_record(p)
 if row:retained_previous.append(row)
 else:direct.append(dict(path=str(p.relative_to(R)),sha256=sha(p)))
need(len(retained_previous)==21,'complete earlier retained archive inventory')
verified=[]
for pattern in ['2026-10-20-optimal-retained-*.json','2026-10-21-optimal-retained-*.json']:
 for p in sorted((R/'docs/evidence').glob(pattern)):
  row=archive_record(p)
  if row:verified.append(row)
  else:direct.append(dict(path=str(p.relative_to(R)),sha256=sha(p)))
need(len(verified)==21 and len({r['report']for r in verified})==21,'exact completed Module20 archive inventory')
p=R/'docs/evidence/2026-10-20-retained-optimal-schedule-review.json';direct.append(dict(path=str(p.relative_to(R)),sha256=sha(p)))
current={}
for name in ['2026-10-20-optimal-retained-public-main','2026-10-21-optimal-retained-public-matrix','2026-10-21-optimal-retained-clockfree','2026-10-21-optimal-retained-frames-timed']:
 e=json.loads((R/'docs/evidence'/(name+'.json')).read_text())
 for n,h in e['source'].items():need(sha(R/n)==h,'current qualified source '+name+':'+n)
 current[name]=dict(files=len(e['source']),source=e['source'])
production={str(p.relative_to(R)):sha(p)for p in sorted((R/'src').rglob('*.rs'))}
for name in ['matrix','platform','native']:
 e=json.loads((T/'retained-public-qualified-gates'/(name+'.json')).read_text());need(e['status']=='passed'and e['source_unchanged']is True and e['production_source_sha256']==production,'exact final production gate '+name)
m=json.loads((T/'retained-public-full-reconciliation/evidence.json').read_text());need(m['status']=='passed'and m['complete_products']==22 and m['no_std_builds']==21 and m['failed_commands']==0,'complete full matrix')
need(len(m['reference_groups'])==69,'complete named group inventory');totals=[sum(g['totals'][i]for g in m['reference_groups'])for i in range(3)];need(totals==[1465,0,55],'exact full outcomes')
s=json.loads((T/'retained-measurement-report/summary.json').read_text());need(s['status']=='passed'and s['case_rows']==598 and s['samples']==2990,'complete timing inventory')
for mode in ['normal','optimized']:
 e=json.loads((T/('retained-timed-controls-'+mode)/'summary.json').read_text());need(e['status']=='passed'and e['checks']==33 and e['rejected']==31 and e['child_attempts']==0 and all(r['passed']is True for r in e['records']),'complete timing controls')
for suffix in ['', '-optimized']:
 e=json.loads((T/('retained-benchmark-controls-v2'+suffix)/'summary.json').read_text());need(e['checks']==e['passed']==43 and e['child_processes']==0 and all(r['passed']is True for r in e['records']),'complete clock-free controls')
extra=['Cargo.lock','Cargo.toml','rust-toolchain.toml','examples/reference_greedy_oracle.rs','test-support/allocator/Cargo.toml','test-support/allocator/src/lib.rs']
native=json.loads((T/'retained-public-capture-v4/evidence.json').read_text())
for n in extra:need(sha(R/n)==native['source'][n]==m['source'][n],'compiler/allocator/native adapter identity')
result=dict(status='passed',recorded_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),module=20,
 scope='Bounded retained no-dictionary Optimal strategy7 through32768bytes, preserving every old fresh request/response. Retained Ultra/Ultra2, dictionaries/prefixes, window expiry, numeric streaming, LDM and workers remain outside this milestone.',
 verified_predecessors=previous,verified_retained_predecessors=retained_previous,verified_evidence=verified,verified_direct_reports=direct,current_source_inventories=current,production_source_sha256=production,additional_production_inputs={n:sha(R/n)for n in extra},
 full_matrix=dict(products=22,groups=69,result_sections=70,per_product_passed=1465,per_product_ignored=55,total_passed_executions=32230,total_ignored_executions=1210,no_std_builds=21,gate_sha256=sha(T/'retained-public-full-reconciliation/evidence.json')),
 measurement=dict(case_rows=598,samples=2990,measured_executables=23,all_safe_ratios=[r['geometric_mean_cost_ratio_to_safe']for r in s['aggregates']if r['config']=='all']),
 qualification='All21Module20archive/report pairs,21earlier retained pairs and68Ultra/Ultra2predecessor pairs retain exact identities. Current full-regression, clock-free and measured source inventories agree; historical private/admission sources are not relabeled as current runs. Final public foreign Miri and concrete no-atomics builds bind86Rustfiles directly, with6compiler/allocator/adapter inputs reconciled separately. Native-to-final Rustdoc/lint-only differences are explicit. All prior failed attempts, including the ineffective initial benchmark control and first matrix group-count reconciliation, remain preserved. Complete upstream migration and its55ignored/188unfinished obligations remain open; no universal unsafe speedup is claimed.',reconciliation_driver_sha256=sha(Path(__file__)))
DEST.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n');print(json.dumps(dict(status='passed',module=20,archives=len(verified),retained_predecessors=len(retained_previous),other_predecessors=len(previous),production=len(production)+len(extra),per_product=totals)))
