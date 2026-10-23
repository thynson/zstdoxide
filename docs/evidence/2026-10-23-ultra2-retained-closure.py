"""Reconcile the bounded retained Ultra2 milestone and its explicitly qualified historical sources."""
from pathlib import Path
import datetime,hashlib,json
R=Path('/Users/lanxingcan/Workspace/zstdoxize');T=Path('/private/tmp');DEST=R/'docs/evidence/2026-10-23-ultra2-retained-closure.json'
def need(v,m):
 if not v:raise ValueError(m)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb')as f:
  while b:=f.read(1<<20):h.update(b)
 return h.hexdigest()
def read(p):return json.loads(p.read_text())
def archive_record(p):
 e=read(p);a=e['archive']
 if isinstance(a,dict):path=R/a['path'];digest=a['sha256'];size=a['bytes']
 else:path=p.parent/a;digest=e['archive_sha256'];size=e['archive_bytes']
 need(path.parent==R/'docs/evidence'and path.is_file()and not path.is_symlink(),'contained archive')
 need(sha(path)==digest and path.stat().st_size==size,'exact archive '+str(path))
 return dict(report=str(p.relative_to(R)),report_sha256=sha(p),archive=str(path.relative_to(R)),archive_sha256=digest,archive_bytes=size)
need(not DEST.exists(),'fresh closure')
predecessor=R/'docs/evidence/2026-10-22-ultra-retained-closure.json';old=read(predecessor);need(old['status']=='passed','completed predecessor')
previous=old['verified_predecessors']+old['verified_evidence'];need(len(previous)==129 and len({r['report']for r in previous})==129,'complete129predecessor archive pairs')
for row in previous:need(sha(R/row['report'])==row['report_sha256']and sha(R/row['archive'])==row['archive_sha256'],'unchanged predecessor')
old_suffixes=['constructors-compact','execution','fixture','frame-preflight','integration','model','native','plan','preflight','private-platform','public-native-preview','public-preflight','public-recipes','public-usage-preview','recipes','search','wire']
new_suffixes=['clockfree','clockfree-v2','final-focused','public-native','public-platform','public-matrix','frames-timed']
reports=[R/'docs/evidence'/('2026-10-22-ultra2-retained-'+n+'.json')for n in old_suffixes]+[R/'docs/evidence'/('2026-10-23-ultra2-retained-'+n+'.json')for n in new_suffixes]
need(len(reports)==24,'exact24module archive pairs');verified=[archive_record(p)for p in reports]
current={}
for name in ['final-focused','public-matrix','clockfree','clockfree-v2','frames-timed']:
 e=read(R/'docs/evidence'/('2026-10-23-ultra2-retained-'+name+'.json'))
 for n,h in e['source'].items():need(sha(R/n)==h,'exact current qualified source '+name+':'+n)
 current[name]=dict(files=len(e['source']),source=e['source'])
binding=read(R/'docs/evidence/2026-10-23-ultra2-retained-final-source-binding.json');need(binding['status']=='passed'and len(binding['differences'])==1,'explicit historical Rustdoc binding')
production={str(p.relative_to(R)):sha(p)for p in sorted((R/'src').rglob('*.rs'))};need(production==binding['production_source_sha256'],'final production binding')
for name in ['matrix','platform','native']:
 e=read(T/'retained-ultra2-public-qualified-gates'/(name+'.json'));need(e['status']=='passed'and e['source_unchanged']is True and e['production_source_sha256']==production,'final production gate '+name)
m=read(T/'retained-ultra2-public-qualified-gates/matrix.json');need(m['complete_products']==22 and m['no_std_builds']==21 and m['isolated_features']==19 and m['failed_commands']==0 and m['ignored_per_product']==55,'complete full matrix')
s=read(T/'retained-ultra2-measurement-report/summary.json');need(s['status']=='passed'and s['case_rows']==598 and s['samples']==2990,'complete actual measurements')
for mode in ['normal','optimized']:
 e=read(T/('retained-ultra2-timed-controls-'+mode)/'summary.json');need(e['status']=='passed'and e['checks']==33 and e['rejected']==31 and e['child_attempts']==0 and all(r['passed']is True for r in e['records']),'complete timed controls')
 e=read(T/'retained-ultra2-clockfree-controls-v2'/mode/'summary.json');need(e['checks']==e['passed']==43 and e['child_processes']==0 and all(r['passed']is True for r in e['records']),'complete clock-free controls')
extra=['Cargo.lock','Cargo.toml','rust-toolchain.toml','examples/reference_greedy_oracle.rs','test-support/allocator/Cargo.toml','test-support/allocator/src/lib.rs'];native=read(T/'retained-ultra2-public-capture/evidence.json')
for n in extra:need(sha(R/n)==native['source'][n]==m['source'][n],'compiler/allocator/adapter identity')
todo=(R/'TODO.md').read_text();section=todo[todo.index('## Module 22:'):todo.index('## Module 23 preparation:')];remaining=[line for line in section.splitlines()if line.startswith('- [ ]')]
need(remaining==['- [ ] Reconcile the bounded milestone, clear its checklist and decompose the next scope without claiming complete upstream parity.'],'all preceding module work complete')
need((R/'docs/caller-output-design.md').is_file(),'analyzed next scope')
result=dict(status='passed',module=22,recorded_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),scope='Bounded retained no-dictionary Ultra2 strategy9 through32768bytes within one window. Fresh frames are unchanged; only exact declared predecessor indices1512/1592 move from resource rejection to success. First block seeds/rebases; later blocks parse once with retained Ultra state. Dictionaries/prefixes, window expiry, numeric streaming, LDM, workers and complete upstream parity remain excluded.',predecessor_closure_sha256=sha(predecessor),verified_predecessors=previous,verified_evidence=verified,current_source_inventories=current,final_source_binding_sha256=sha(R/'docs/evidence/2026-10-23-ultra2-retained-final-source-binding.json'),historical_source_differences=binding['differences'],production_source_sha256=production,additional_production_inputs={n:sha(R/n)for n in extra},full_matrix=dict(products=22,no_std_builds=21,per_product_passed=m['passed_per_product'],per_product_ignored=55,result_sections=m['result_sections'],named_outcomes=m['named_outcomes'],total_passed_executions=22*m['passed_per_product'],total_ignored_executions=1210,gate_sha256=sha(T/'retained-ultra2-public-qualified-gates/matrix.json')),measurement=dict(case_rows=598,samples=2990,measured_executables=23,all_safe_ratios=[r['geometric_mean_cost_ratio_to_safe']for r in s['aggregates']if r['config']=='all']),next_scope_design_sha256=sha(R/'docs/caller-output-design.md'),qualification='All24Module22archive/report pairs and129predecessor pairs retain exact identities. Final focused/matrix/measured sources agree. Native/foreign sources have an explicit Rustdoc-only reconciliation with identical executable Rust; no historical run is relabeled. Prior controller/test-expectation/archive/disk failures remain preserved. Default builds are safe Rust; nineteen opt-in storage features are independently qualified. No universal unsafe speedup or complete upstream parity is claimed;55ignored tests and188unfinished source units remain open.',reconciliation_driver_sha256=sha(Path(__file__)))
DEST.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n');print(json.dumps(dict(status='passed',module=22,archives=24,predecessors=129,production_inputs=len(production)+len(extra),per_product_passed=m['passed_per_product'],per_product_ignored=55),sort_keys=True))
