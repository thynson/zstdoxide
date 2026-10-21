#!/usr/bin/env python3
"""Read-only replay controls for actual retained Rust wire histories."""
import argparse,copy,json,shutil,subprocess,tempfile,sys
from pathlib import Path
import audit_ultra_retained_rust_frames as validator

def check(source):
    children=[]
    def blocked(*args,**kwargs):children.append((args,kwargs));raise RuntimeError('child forbidden')
    subprocess.run=subprocess.Popen=subprocess.call=subprocess.check_call=subprocess.check_output=blocked
    def guard(event,args):
        if event in ("subprocess.Popen","os.system","os.fork","os.posix_spawn"):blocked(event,args)
    sys.addaudithook(guard)
    baseline=validator.audit(source);negative=0
    with tempfile.TemporaryDirectory(prefix='retained-wire-controls-')as directory:
        root=Path(directory)
        for p in source.iterdir():
            if p.is_file():shutil.copyfile(p,root/p.name)
        original=(root/'observations.json').read_bytes();rows=json.loads(original)
        mutations=[('incoming',lambda b:b['incoming'].__setitem__(0,2)),
          ('candidate',lambda b:b['candidate'].__setitem__(1,99)),
          ('committed',lambda b:b['committed'].__setitem__(0,99)),
          ('kind',lambda b:b.__setitem__('kind',(b['kind']+1)%3)),
          ('start',lambda b:b.__setitem__('start',b['start']+1)),
          ('length',lambda b:b.__setitem__('length',b['length']-1)),
          ('tail',lambda b:b.__setitem__('tail',b['tail']+1)),
          ('bool',lambda b:b.__setitem__('start',False)),
          ('extra',lambda b:b.__setitem__('extra',0)),
          ('literal',lambda b:b['literals'].__setitem__(0,b['literals'][0]^1)),
          ('sequence',lambda b:b['sequences'][0].__setitem__(2,b['sequences'][0][2]+1)),
          ('offset',lambda b:b['sequences'][0].__setitem__(3,b['sequences'][0][3]+1))]
        def reject(label):
            nonlocal negative
            try:validator.audit(root)
            except (ValueError,AssertionError,KeyError,TypeError,IndexError,UnicodeError,json.JSONDecodeError):negative+=1
            else:raise ValueError('accepted mutation '+label)
        for block_index in [0, 1]:
            for label,mutation in mutations:
                # Uniform retained blocks always contain a match; later blocks
                # may contain no literals, so select an effective array forgery.
                if label=='literal' and not rows[0]['blocks'][block_index]['literals']:
                    mutation=lambda b:b['literals'].append(0)
                changed=copy.deepcopy(rows);mutation(changed[0]['blocks'][block_index])
                if json.dumps(changed,sort_keys=True,separators=(',',':'))==json.dumps(rows,sort_keys=True,separators=(',',':')):raise ValueError('ineffective control '+label)
                (root/'observations.json').write_text(json.dumps(changed));reject(str(block_index)+'-'+label)
        for label,changed in [('missing',rows[:-1]),('duplicate',rows[:-1]+rows[:1]),('order',list(reversed(rows)))]:
            (root/'observations.json').write_text(json.dumps(changed));reject(label)
        (root/'observations.json').write_bytes(original)
        for suffix in ['raw','zst']:
            p=root/(rows[0]['name']+'.'+suffix);saved=p.read_bytes()
            for label,data in [('truncated',saved[:-1]),('changed',bytes((saved[0]^1,))+saved[1:]),('trailing',saved+b'\0')]:
                p.write_bytes(data);reject(suffix+'-'+label)
            p.write_bytes(saved)
        extra=root/'unexpected.bin';extra.write_bytes(b'extra');reject('unexpected artifact');extra.unlink()
        if validator.audit(root)!=baseline:raise ValueError('restored baseline')
    if children:raise ValueError('no child invocation')
    return dict(status='passed',negative_controls=negative,positive_controls=2,child_processes=0,baseline_audit=baseline)
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('capture',type=Path);a=parser.parse_args();print(json.dumps(check(a.capture),sort_keys=True))
