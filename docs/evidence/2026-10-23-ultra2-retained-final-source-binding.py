from pathlib import Path
import hashlib,json
R=Path('/Users/lanxingcan/Workspace/zstdoxize');T=Path('/private/tmp')
def need(v,m):
 if not v:raise ValueError(m)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
paths={name:T/path for name,path in [('historical_focused','retained-ultra2-public-qualification-v3'),('final_focused','retained-ultra2-public-qualification-v4'),('native','retained-ultra2-public-capture'),('audits','retained-ultra2-public-native-audits'),('platform','retained-ultra2-public-platform')]}
e={name:json.loads((p/'evidence.json').read_text())for name,p in paths.items()}
for name,v in e.items():need(v['status']=='passed'and v['source_unchanged'],'completed '+name)
old=e['historical_focused']['source'];new=e['final_focused']['source'];differences=[]
for n,h in old.items():
 need(n in new,'preserved historical source path '+n)
 if h!=new[n]:
  need(n=='src/reference_encode.rs','only documented public Rustdoc correction')
  a=(paths['historical_focused']/'source'/n).read_text();b=(paths['final_focused']/'source'/n).read_text()
  strip=lambda text:'\n'.join(line for line in text.splitlines()if not line.lstrip().startswith('///'))
  need(strip(a)==strip(b),'identical executable Rust lines')
  differences.append(dict(path=n,before=h,after=new[n],qualification='Only /// Rustdoc lines differ; all remaining lines are identical. Historical tests are not relabeled as using current documentation.'))
for name in ['native','audits','platform']:need(e[name]['source']==old,'exact original native/platform inventory '+name)
for n,h in new.items():need(sha(R/n)==h==sha(paths['final_focused']/'source'/n),'current final focused identity '+n)
need(e['audits']['capture_evidence_sha256']==sha(paths['native']/'evidence.json'),'audited actual capture')
for name,key in [('native','base_evidence_sha256'),('audits','qualified_evidence_sha256'),('platform','base_evidence_sha256')]:need(e[name][key]==sha(paths['historical_focused']/'evidence.json'),'original focused binding '+name)
need(len(e['platform']['commands'])==8 and e['platform']['interpreted_test_executions']==4 and e['platform']['concrete_codegen_builds']==2,'complete foreign gates')
for row in e['platform']['commands']:need(row['exit_code']==0 and sha(paths['platform']/row['log']['path'])==row['log']['sha256'],'bound successful platform log')
for rows in e['platform']['concrete_artifacts'].values():
 for row in rows:need(sha(paths['platform']/row['path'])==row['sha256'],'exact retained foreign artifact')
result=dict(status='passed',source_unchanged=True,source=new,production_source_sha256={n:h for n,h in new.items()if n.startswith('src/')},differences=differences,added_final_inputs={n:h for n,h in new.items()if n not in old},receipts={name:sha(p/'evidence.json')for name,p in paths.items()},qualification='Public native interoperability and final entry-point foreign Miri/codegen bind identical executable Rust. Only the documented algorithm Rustdoc differs in the final focused source. New final inputs are identified separately. Full feature regression and benchmarks remain distinct gates.')
out=R/'docs/evidence/2026-10-23-ultra2-retained-final-source-binding.json';need(not out.exists(),'fresh final binding');out.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n');print('Reconciled',len(new),'inputs and',len(differences),'explicit Rustdoc-only delta.')
