"""Reconcile archived Ultra2 gates and exactly unchanged qualified sources."""
import datetime,hashlib,json
from pathlib import Path
R=Path('/Users/lanxingcan/Workspace/zstdoxize');T=Path('/private/tmp')
def sha(p):
 h=hashlib.sha256()
 with p.open('rb')as f:
  for b in iter(lambda:f.read(1<<20),b''):h.update(b)
 return h.hexdigest()
def need(x,m):
 if not x:raise ValueError(m)
old=json.loads((R/'docs/evidence/2026-10-20-ultra-closure.json').read_text());previous=[]
for row in old['verified_evidence']:
 need(sha(R/row['report'])==row['report_sha256'] and sha(R/row['archive'])==row['archive_sha256'],'unchanged predecessor identity');previous.append(row)
verified=[]
for p in sorted((R/'docs/evidence').glob('*ultra2*.json')):
 d=json.loads(p.read_text())
 if 'archive_sha256'not in d:continue
 a=p.parent/d['archive'];need(a.parent==p.parent and d['status']=='passed','passed local archive');need(sha(a)==d['archive_sha256'] and a.stat().st_size==d['archive_bytes'],'archive identity')
 verified.append(dict(report=str(p.relative_to(R)),report_sha256=sha(p),archive=str(a.relative_to(R)),archive_sha256=sha(a),archive_bytes=a.stat().st_size))
need(len(previous)==45 and len(verified)==23,'exact completed archive inventories')
current={}
for stage in ['public-matrix','clockfree','frames-timed']:
 p=R/('docs/evidence/2026-10-20-ultra2-'+stage+'.json');d=json.loads(p.read_text())
 for n,h in d['source'].items():need(sha(R/n)==h,'unchanged qualified source '+stage+':'+n)
 current[stage]=dict(files=len(d['source']),source=d['source'])
production=json.loads((R/'tests/fixtures/reference-ultra2-interop-recipes/production-sources.json').read_text())['source_sha256']
for n,h in production.items():need(sha(R/n)==h,'current public production '+n)
m=json.loads((T/'ultra2-public-qualified-gates/matrix.json').read_text());need(m['status']=='passed' and m['complete_products']==22 and m['no_std_builds']==21 and m['failed_commands']==0 and m['production_source_sha256']==production,'full matrix gate')
need(len(m['reference_groups'])==67,'complete independent named group inventory')
totals=[sum(g['totals'][i]for g in m['reference_groups'])for i in range(3)];need(totals==[1435,0,55],'full matrix totals')
s=json.loads((T/'ultra2-measurement-report/summary.json').read_text());need(s['status']=='passed' and s['case_rows']==598 and s['samples']==2990,'timing inventory')
for mode in ['normal','optimized']:
 c=json.loads((T/('ultra2-timed-controls-'+mode)/'summary.json').read_text());need(c['status']=='passed' and c['checks']==33 and c['rejected']==31 and c['child_attempts']==0 and all(r['passed']is True for r in c['records']),'timing controls')
result=dict(status='passed',recorded_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),module=19,
 scope='Bounded fresh no-dictionary Ultra2 strategy9; retained public blocks, dictionary/prefix, window expiry, numeric streaming, LDM and workers remain outside this milestone.',
 verified_predecessors=previous,verified_evidence=verified,current_source_inventories=current,production_source_sha256=production,
 full_matrix=dict(products=22,groups=67,per_product_passed=1435,per_product_ignored=55,no_std_builds=21,gate_sha256=sha(T/'ultra2-public-qualified-gates/matrix.json')),
 measurement=dict(case_rows=598,samples=2990,all_safe_ratios=[r['geometric_mean_cost_ratio_to_safe']for r in s['aggregates']if r['config']=='all']),
 qualification='All23Ultra2archives and45predecessorarchive/report pairs remain intact. Full regression, clock-free and measured inventories are exactly current; earlier private/admission stage inventories remain historical and are not relabeled as new runs. Formatting, Clippy, platform and native evidence qualify the unchanged90production identities. The corrected full-regression empty-source expectation and all prior failed/cancelled attempts remain explicit. There is no stable unsafe speedup or complete upstream migration claim. Next work is independently qualifying larger retained Optimal sources before any production widening.',
 reconciliation_driver_sha256=sha(Path(__file__)))
p=R/'docs/evidence/2026-10-20-ultra2-closure.json';need(not p.exists(),'fresh closure');p.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n');print(json.dumps(dict(status='passed',archives=len(verified),predecessors=len(previous),source_pins=sum(v['files']for v in current.values()),production=len(production),per_product=totals)))
