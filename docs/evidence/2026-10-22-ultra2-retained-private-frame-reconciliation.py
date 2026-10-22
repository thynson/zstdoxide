"""Bind all completed private retained Ultra2 frame gates before public admission."""
from pathlib import Path
import hashlib,json
R=Path('/Users/lanxingcan/Workspace/zstdoxize');T=Path('/private/tmp')
def need(v,m):
 if not v:raise ValueError(m)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
specs={'integration':('retained-ultra2-integration-rust-full','2026-10-22-ultra2-retained-integration',49),'wire':('retained-ultra2-wire-qualification','2026-10-22-ultra2-retained-wire',67),'platform':('retained-ultra2-frame-platform','2026-10-22-ultra2-retained-private-platform',8)}
stages={};archive_records={}
for name,(directory,artifact,count)in specs.items():
 root=T/directory;e=json.loads((root/'evidence.json').read_text());need(e['status']=='passed'and e['source_unchanged']and len(e['commands'])==count and all(c['exit_code']==0 for c in e['commands']),'complete '+name)
 for n,h in e['source'].items():need(sha(R/n)==h==sha(root/'source'/n),'current qualified '+name+' source '+n)
 for c in e['commands']:
  for key in ['stdout','stderr','log']:
   if key in c:need(sha(root/c[key]['path'])==c[key]['sha256'],'bound '+name+' command output')
 receipt=R/'docs/evidence'/(artifact+'.json');archive=R/'docs/evidence'/(artifact+'.tar.gz');a=json.loads(receipt.read_text());need(a['status']=='passed'and sha(archive)==a['archive_sha256'],'exact completed '+name+' archive')
 stages[name]=e;archive_records[name]=dict(receipt=receipt.name,receipt_sha256=sha(receipt),archive=archive.name,archive_sha256=sha(archive),stage_evidence_sha256=sha(root/'evidence.json'))
base=stages['integration']['source'];production={n:h for n,h in base.items()if n.startswith('src/')}
for name in ['wire','platform']:
 e=stages[name];need(e['base_evidence_sha256']==archive_records['integration']['stage_evidence_sha256'],'exact integration binding '+name)
 need({n:h for n,h in e['source'].items()if n.startswith('src/')}==production,'exact production closure '+name)
 need(all(e['source'].get(n)==h for n,h in base.items()),'complete integration input inclusion '+name)
need(len([c for c in stages['integration']['commands']if c['label'].startswith('test-')and len(c.get('tests',[]))==13])==22,'22 complete focused products')
need(len([c for c in stages['integration']['commands']if c['label'].startswith('no-std-')])==21,'21 no_std products')
wire=stages['wire'];audit=wire['independent_audit'];need(audit['negative_controls']==34 and audit['positive_controls']==2 and audit['child_processes']==0 and len(wire['capture'])==59,'complete independent wire outcomes')
for name,row in wire['capture'].items():
 for mode in ['safe','all']:need(sha(T/specs['wire'][0]/'captures'/mode/name)==row['sha256'],'unchanged wire '+mode+' '+name)
for rows in stages['platform']['concrete_artifacts'].values():
 for row in rows:need(sha(T/specs['platform'][0]/row['path'])==row['sha256'],'concrete artifact identity')
# Keep the public resolver closed through this private checkpoint.
search=(R/'src/reference_search.rs').read_text();need('''            ReferenceStrategy::Ultra2 => {
                Ultra2FramePlan::new(parameters, source_len).map(Self::Ultra2)
            }'''in search,'private checkpoint preserves fresh-only public resolver')
output=R/'docs/evidence/2026-10-22-ultra2-retained-private-frame-reconciliation.json';need(not output.exists(),'fresh reconciliation')
record=dict(status='passed',source_unchanged=True,controller_sha256=sha(Path(__file__)),production_source_sha256=production,archives=archive_records,commands=124,focused_test_executions=286,public_predecessor_test_executions=18,true_no_std_products=21,wire_cases=29,capture_artifacts_per_build=59,native_decodes=58,independent_audit=audit['baseline_audit'],negative_controls_per_interpreter=34,positive_controls_per_interpreter=2,miri_test_executions=12,platform_commands=8,public_retained_ultra2_admitted=False,qualification='All private retained Ultra2 token/frame integration, actual-wire independent replay/native decoding and foreign platform gates bind the same complete production sources and integration inputs. Every referenced log, archive, capture and concrete platform artifact is checked again. Public resolver remains fresh-only; source-only public hypotheses are a separate completed plan. Public admission, final full regression, final platform binding, benchmarks and complete upstream parity remain open.')
output.write_text(json.dumps(record,sort_keys=True,indent=2)+'\n');print(json.dumps(dict(status='passed',production_files=len(production),commands=124,archives=archive_records),sort_keys=True))
