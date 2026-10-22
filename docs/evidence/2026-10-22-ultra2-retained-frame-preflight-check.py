"""Bind every preflight command log to its original, full and compact copies."""
from pathlib import Path
import hashlib,json
R=Path('/Users/lanxingcan/Workspace/zstdoxize');T=Path('/private/tmp');name='2026-10-22-ultra2-retained-frame-preflight';B=T/'retained-ultra2-frame-preflight-bundle';C=T/(name+'-compact-bundle')
def need(v,m):
 if not v:raise ValueError(m)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
checks=[]
for label in ['integration-preview-1','integration-preview-unoptimized-1','wire-preview-1','wire-preview-native-1','wire-preview-optimized-1']:
 D=T/('retained-ultra2-'+label);e=json.loads((D/'evidence.json').read_text());need(e['status']=='passed','completed experiment')
 if label=='integration-preview-1':pairs=list(zip(['test','clippy'],e['commands']))
 elif label=='integration-preview-unoptimized-1':pairs=[('test',e['commands'][0])]
 elif label=='wire-preview-optimized-1':pairs=[('controls',e)]
 else:pairs=[(c['label'],c)for c in e['commands']]
 for command,row in pairs:
  need(row['exit_code']==0,'successful recorded command')
  for stream in ['stdout','stderr']:
   filename=command+'.'+stream;expected=row[stream+'_sha256']
   for root in [D,B/'experiments'/label,C/'experiments'/label]:need(sha(root/filename)==expected,'exact logged command output '+filename)
   checks.append(dict(experiment=label,file=filename,sha256=expected))
need(len(checks)==80,'forty commands and eighty logs')
p=R/'docs/evidence'/(name+'.json');a=json.loads(p.read_text());need(a['status']=='passed'and sha(R/'docs/evidence'/a['archive'])==a['archive_sha256'],'exact completed archive')
report=dict(status='passed',qualification='Additional read-only digest verification of all 80 command logs in original, full and compact experiment copies. No codec or interpreter test is claimed rerun. The preflight remains unpromoted.',receipt_sha256=sha(p),archive_sha256=a['archive_sha256'],commands=40,logs=checks)
out=R/'docs/evidence'/(name+'-check.json');need(not out.exists(),'fresh digest receipt');out.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n');print('80 command logs and final archive identity verified')
