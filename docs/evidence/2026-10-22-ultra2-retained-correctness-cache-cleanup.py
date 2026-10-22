"""Remove only task-owned ordinary caches after checking recorded binary identities."""
from pathlib import Path
import hashlib,json,shutil
R=Path('/Users/lanxingcan/Workspace/zstdoxize');T=Path('/private/tmp')
def need(v,m):
 if not v:raise ValueError(m)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
w=T/'retained-ultra2-wire-qualification';n=T/'retained-ultra2-public-native-preview';wire=json.loads((w/'evidence.json').read_text());native=json.loads((n/'evidence.json').read_text());need(wire['status']==native['status']=='passed','completed cache users')
archives={}
for name in ['2026-10-22-ultra2-retained-wire','2026-10-22-ultra2-retained-public-native-preview']:
 p=R/'docs/evidence'/(name+'.json');a=json.loads(p.read_text());need(a['status']=='passed'and sha(R/'docs/evidence'/a['archive'])==a['archive_sha256'],'verified archive');archives[name]=a['archive_sha256']
verified=[]
for c in wire['commands']:
 if c['label']in ['safe-capture','all-capture']:
  for b in c['executables']:need(sha(Path(b['path']))==b['sha256'],'wire executable identity');verified.append(b)
need(len(verified)==2,'two actual wire test executables')
for mode in ['safe','all']:
 p=T/('retained-ultra2-public-native-preview-'+mode)/'release/examples/reference_greedy_oracle';q=n/'binaries'/('rust-'+mode);h=native['binaries']['binaries/rust-'+mode];need(sha(p)==h==sha(q),'retained exact public candidate binary');verified.append(dict(path=str(p),sha256=h,retained_copy=str(q)))
roots=[T/'retained-ultra2-wire-target',*[T/('retained-ultra2-public-native-preview-'+m)for m in ['safe','all']]]
record=dict(status='verified',archives=archives,executables=verified,removed=[],qualification='Only three task-owned ordinary correctness Cargo caches are removed after all callers finished and actual binary hashes matched completed evidence. Candidate interop executables remain in the native-preview stage and archive. Wire test executable identities remain recorded with full source. No benchmark executable, Miri sysroot, source snapshot, captured artifact or log is removed.')
out=R/'docs/evidence/2026-10-22-ultra2-retained-correctness-cache-cleanup.json';need(not out.exists(),'fresh cleanup receipt');out.write_text(json.dumps(record,sort_keys=True,indent=2)+'\n')
for root in roots:
 need(root.is_dir()and not root.is_symlink()and root.resolve()==root and root.parent==T,'exact task-owned cache')
 files=[p for p in root.rglob('*')if p.is_file()and not p.is_symlink()];row=dict(path=str(root),files=len(files),bytes=sum(p.stat().st_size for p in files));shutil.rmtree(root);need(not root.exists(),'cache removed');record['removed'].append(row);out.write_text(json.dumps(record,sort_keys=True,indent=2)+'\n')
record['status']='passed';record['controller_sha256']=sha(Path(__file__));out.write_text(json.dumps(record,sort_keys=True,indent=2)+'\n');print(json.dumps(dict(status='passed',removed_bytes=sum(r['bytes']for r in record['removed']),verified_executables=len(verified)),sort_keys=True))
