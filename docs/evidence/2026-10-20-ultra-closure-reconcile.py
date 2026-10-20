import datetime,hashlib,json
from pathlib import Path
R=Path('/Users/lanxingcan/Workspace/zstdoxize')
def sha(p):
 h=hashlib.sha256()
 with p.open('rb')as f:
  for b in iter(lambda:f.read(1<<20),b''):h.update(b)
 return h.hexdigest()
def need(v,m):
 if not v:raise ValueError(m)
verified=[]
for p in sorted((R/'docs/evidence').glob('*ultra*.json')):
 d=json.loads(p.read_bytes())
 if 'archive_sha256'not in d:continue
 a=p.parent/d['archive'];need(a.parent==p.parent and a.is_file(),'local archive');need(sha(a)==d['archive_sha256'],'archive digest '+a.name)
 if 'archive_bytes'in d:need(a.stat().st_size==d['archive_bytes'],'archive extent')
 verified.append(dict(report=str(p.relative_to(R)),report_sha256=sha(p),archive=str(a.relative_to(R)),archive_sha256=d['archive_sha256']))
sources={}
for name in ['2026-10-19-ultra-public-matrix','2026-10-19-ultra-clockfree','2026-10-20-ultra-frames-timed','2026-10-20-ultra-closure-checks']:
 p=R/'docs/evidence'/(name+'.json');d=json.loads(p.read_bytes());need(d['status']=='passed','qualified report')
 for n,h in d['source'].items():need(sha(R/n)==h,'unchanged current source '+n)
 sources[name]=dict(files=len(d['source']),source=d['source'])
checks=json.loads(Path('/private/tmp/ultra-closure-checks-1020/evidence.json').read_bytes());need(len(checks['commands'])==7 and all(c['exit_code']==0 for c in checks['commands']),'final focused checks')
summary=json.loads(Path('/private/tmp/ultra-measurement-report/summary.json').read_bytes());need(summary['case_rows']==598 and summary['samples']==2990,'complete measured inventory')
for mode in ['normal','optimized']:
 c=json.loads(Path('/private/tmp/ultra-timed-controls-'+mode+'/summary.json').read_bytes());need(c['checks']==33 and c['rejected']==31 and c['child_attempts']==0 and all(r['passed']is True for r in c['records']),'timing controls')
result=dict(status='passed',recorded_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),module=18,scope='Bounded fresh no-dictionary Ultra strategy 8; Ultra2 strategy 9 remains unsupported.',verified_evidence=verified,current_source_inventories=sources,final_focused_commands=checks['commands'],measurement=dict(case_rows=598,samples=2990,all_safe_ratios=[r['geometric_mean_cost_ratio_to_safe']for r in summary['aggregates']if r['config']=='all']),qualification='Archive identities and unchanged tested/measured sources are verified. Historical checkpoint scopes, expected nonzero commands, interrupted campaigns and six original full-regression timeouts are retained; this reconciliation does not promote them to uninterrupted successes. Complete official test migration remains open.',reconciliation_driver_sha256=sha(Path(__file__)))
p=R/'docs/evidence/2026-10-20-ultra-closure.json';need(not p.exists(),'fresh closure');p.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n');print('Verified',len(verified),'archive/report pairs and',sum(v['files']for v in sources.values()),'source pins.')
