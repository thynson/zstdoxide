"""Reconcile the bounded retained Ultra milestone without relabeling historical runs."""
from pathlib import Path
import datetime,hashlib,json
R=Path('/Users/lanxingcan/Workspace/zstdoxize');T=Path('/private/tmp');DEST=R/'docs/evidence/2026-10-22-ultra-retained-closure.json'
def need(v,m):
 if not v:raise ValueError(m)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb')as f:
  for b in iter(lambda:f.read(1<<20),b''):h.update(b)
 return h.hexdigest()
def read(p):return json.loads(p.read_text())
def archive_record(p):
 e=read(p);a=e['archive']
 if type(a)is dict:path=R/a['path'];digest=a['sha256'];size=a['bytes']
 else:path=p.parent/a;digest=e['archive_sha256'];size=e['archive_bytes']
 need(path.parent==R/'docs/evidence'and path.is_file()and not path.is_symlink(),'local archive')
 need(sha(path)==digest and path.stat().st_size==size,'exact archive '+str(path))
 return dict(report=str(p.relative_to(R)),report_sha256=sha(p),archive=str(path.relative_to(R)),archive_sha256=digest,archive_bytes=size)
need(not DEST.exists(),'fresh closure')
predecessor=R/'docs/evidence/2026-10-21-optimal-retained-closure.json';old=read(predecessor);need(old['status']=='passed','completed predecessor')
previous=old['verified_predecessors']+old['verified_retained_predecessors']+old['verified_evidence'];need(len(previous)==110 and len({r['report']for r in previous})==110,'complete predecessor archive inventory')
for row in previous:need(sha(R/row['report'])==row['report_sha256']and sha(R/row['archive'])==row['archive_sha256'],'unchanged predecessor archive/report')
suffixes=['model','recipes','preflight','native','fixture','reader','scope','plan','execution','state','frame','rust-wire','private-platform','public-recipes','public-native','public-platform','public-matrix','clockfree','frames-timed']
reports=[R/'docs/evidence'/(('2026-10-22' if n=='frames-timed' else '2026-10-21')+'-ultra-retained-'+n+'.json')for n in suffixes];need(len(reports)==19,'exact module archive inventory');verified=[archive_record(p)for p in reports]
current={}
for name in ['public-native','public-platform','public-matrix','clockfree','frames-timed']:
 e=read(R/'docs/evidence'/(('2026-10-22' if name=='frames-timed' else '2026-10-21')+'-ultra-retained-'+name+'.json'))
 for n,h in e['source'].items():need(sha(R/n)==h,'current qualified source '+name+':'+n)
 current[name]=dict(files=len(e['source']),source=e['source'])
production={str(p.relative_to(R)):sha(p)for p in sorted((R/'src').rglob('*.rs'))};need(len(production)==86,'production Rust inventory')
for name in ['matrix','platform','native']:
 e=read(T/'retained-ultra-public-qualified-gates'/(name+'.json'));need(e['status']=='passed'and e['source_unchanged']is True and e['production_source_sha256']==production,'exact final production '+name)
m=read(T/'retained-ultra-public-qualified-gates/matrix.json');need(m['complete_products']==22 and m['no_std_builds']==21 and m['isolated_features']==19 and m['failed_commands']==0 and m['ignored_per_product']==55,'complete full matrix')
s=read(T/'retained-ultra-measurement-report/summary.json');need(s['status']=='passed'and s['case_rows']==598 and s['samples']==2990,'complete timing inventory')
for mode in ['normal','optimized']:
 e=read(T/('retained-ultra-timed-controls-'+mode)/'summary.json');need(e['status']=='passed'and e['checks']==33 and e['rejected']==31 and e['child_attempts']==0 and all(r['passed']is True for r in e['records']),'complete timing controls')
 e=read(T/'retained-ultra-clockfree-controls'/mode/'summary.json');need(e['checks']==e['passed']==43 and e['child_processes']==0 and all(r['passed']is True for r in e['records']),'complete clock-free controls')
extra=['Cargo.lock','Cargo.toml','rust-toolchain.toml','examples/reference_greedy_oracle.rs','test-support/allocator/Cargo.toml','test-support/allocator/src/lib.rs'];native=read(T/'retained-ultra-public-capture/evidence.json')
for n in extra:need(sha(R/n)==native['source'][n]==m['source'][n],'compiler/allocator/adapter binding')
todo=(R/'TODO.md').read_text();section=todo[todo.index('## Module 21:'):todo.index('## Roadmap')];remaining=[line for line in section.splitlines()if line.startswith('- [ ]')];need(remaining==['- [ ] Reconcile the bounded milestone, clear its completed checklist and decompose the next scope without claiming complete upstream parity.'],'every preceding work item complete')
result=dict(status='passed',module=21,recorded_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),scope='Bounded retained no-dictionary Ultra strategy 8 through 32768 bytes within one window. Every old response remains unchanged except the exact predeclared retained-resource-ultra transition. Retained Ultra2, dictionaries/prefixes, window expiry, numeric streaming, LDM, workers and complete upstream parity remain excluded.',predecessor_closure_sha256=sha(predecessor),verified_predecessors=previous,verified_evidence=verified,current_source_inventories=current,production_source_sha256=production,additional_production_inputs={n:sha(R/n)for n in extra},full_matrix=dict(products=22,no_std_builds=21,per_product_passed=m['passed_per_product'],per_product_ignored=55,result_sections=m['result_sections'],named_outcomes=m['named_outcomes'],total_passed_executions=22*m['passed_per_product'],total_ignored_executions=1210,gate_sha256=sha(T/'retained-ultra-public-qualified-gates/matrix.json')),measurement=dict(case_rows=598,samples=2990,measured_executables=23,all_safe_ratios=[r['geometric_mean_cost_ratio_to_safe']for r in s['aggregates']if r['config']=='all']),qualification='All 19 Module 21 archive/report pairs and 110 predecessor pairs retain exact identities. Current native/full-regression/platform and measured inputs agree. Historical helper failures remain in immutable evidence, including public recipe draft errors and the empty allocator doctest controller correction; no inherited check is relabeled as a rerun. Default implementation is safe Rust, with all nineteen opt-in storage features qualified separately. No universal unsafe speedup or complete upstream migration is claimed. The 55 ignored tests and 188 unfinished migration units remain open.',reconciliation_driver_sha256=sha(Path(__file__)))
DEST.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n');print(json.dumps(dict(status='passed',module=21,archives=19,predecessors=110,production_inputs=92,per_product_passed=m['passed_per_product'],per_product_ignored=55),sort_keys=True))
