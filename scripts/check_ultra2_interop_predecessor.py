#!/usr/bin/env python3
"""Reconstruct original encoded-response identities and compare a successor capture."""
from pathlib import Path
import argparse, hashlib, json, sys, tarfile, tempfile
ROOT = Path(__file__).resolve().parents[1]
ARCHIVE = 'docs/evidence/2026-10-19-ultra-public-interop.tar.gz'
ARCHIVE_SHA256 = 'fc99f273d801db8319a55a0352d9a8cd1ad4588494a4d3a5e4e404881ee5d073'
OLD = 'tests/fixtures/reference-ultra-interop-recipes/recipes.json'
OLD_SHA256 = 'b498c6932e2fd85712608833dd7c745945cbea87d53bdfd009e2dae8a295fcd0'
FIXTURE = ROOT / 'tests/fixtures/reference-ultra2-interop-recipes/predecessor-frames.json'
def need(value, reason):
    if not value: raise ValueError(reason)
def sha(data): return hashlib.sha256(data).hexdigest()
def encode(value): return (json.dumps(value, sort_keys=True, indent=2)+'\n').encode()
def regular(path, bound):
    need(path.is_file() and not path.is_symlink() and path.stat().st_size <= bound, 'bounded regular file')
    return path.read_bytes()
def expected():
    archive=ROOT/ARCHIVE;need(sha(regular(archive,64<<20))==ARCHIVE_SHA256,'immutable predecessor archive')
    original=regular(ROOT/OLD,2<<20);need(sha(original)==OLD_SHA256,'immutable predecessor recipes')
    with tarfile.open(archive,'r:gz') as tar:
        member=tar.getmember('file-map.json');need(member.isfile() and member.size==12996927,'bounded archived file map')
        entries=json.load(tar.extractfile(member))
    rows=[]
    for index,case in enumerate(json.loads(original)['recipes']):
        if case['expected']!='success':continue
        for ending in ['rust-encode-safe','rust-encode-all','c-encode-normal','c-encode-sanitized']:
            name=f'logs/case-{index:03}-{ending}.stdout';row=entries[name]
            rows.append(dict(path=name,bytes=row['bytes'],sha256=row['sha256']))
    need(len(rows)==4888,'complete original successful response inventory')
    return dict(schema_version=1,archive=ARCHIVE,archive_sha256=ARCHIVE_SHA256,predecessor_recipe_sha256=OLD_SHA256,successful_predecessor_cases=1222,responses=rows,qualification='Exact original encoded response identities, containing complete frame bytes and observed response metadata, extracted from the preserved lossless audited predecessor archive. Only originally successful requests are compared.')
def check(capture=None, baseline=FIXTURE, model=None):
    model=expected() if model is None else model
    need(regular(baseline,2<<20)==encode(model),'exact independently reconstructed predecessor identities')
    if capture is not None:
        root=capture.resolve(strict=True)
        for row in model['responses']:
            path=root/row['path'];need(path.resolve().is_relative_to(root),'contained response')
            data=regular(path,row['bytes'])
            need(len(data)==row['bytes'] and sha(data)==row['sha256'],'original complete encoded response changed: '+row['path'])
    return dict(status='passed',original_successes=1222,encoded_responses=4888,compared_capture=capture is not None,archive_sha256=ARCHIVE_SHA256)
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--capture',type=Path);parser.add_argument('--controls',action='store_true');args=parser.parse_args()
    children=[]
    def guard(event, arguments):
        if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn'):
            children.append(event);raise ValueError('no child execution')
    sys.addaudithook(guard)
    model=expected();result=check(args.capture,model=model);rejected=[]
    if args.controls:
        with tempfile.TemporaryDirectory(prefix='ultra2-predecessor-controls-') as folder:
            target=Path(folder)/'baseline.json'
            for kind in ['missing-response','changed-frame','changed-archive','float-schema']:
                bad=json.loads(encode(model))
                if kind=='missing-response':bad['responses'].pop()
                elif kind=='changed-frame':bad['responses'][0]['sha256']='0'*64
                elif kind=='changed-archive':bad['archive_sha256']='0'*64
                else:bad['schema_version']=1.0
                target.write_bytes(encode(bad))
                try:check(baseline=target,model=model)
                except ValueError:rejected.append(kind)
                else:raise ValueError('accepted forged predecessor '+kind)
            need(len(rejected)==4,'complete baseline controls')
    need(not children,'child-free predecessor audit');result.update(rejected=rejected,child_processes=0);print(json.dumps(result,sort_keys=True))
if __name__=='__main__':main()
