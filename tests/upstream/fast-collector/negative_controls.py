#!/usr/bin/env python3
"""Packet, observation, and protected-output rejection controls; no codec edits."""
from pathlib import Path
import argparse, copy, hashlib, importlib.util, json, os, struct, subprocess, sys
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent

def require(ok,reason):
    if not ok:raise RuntimeError(reason)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,value):p.write_text(json.dumps(value,sort_keys=True,indent=2)+'\n')
def subject():
    spec=importlib.util.spec_from_file_location('collector_subject',HERE/'collector.py')
    m=importlib.util.module_from_spec(spec);sys.modules[spec.name]=m;spec.loader.exec_module(m);return m

def child(args):
    m=subject();case=next(r for r in m.recipes() if r.name==args.case)
    try:m.validate(case,args.record)
    except (ValueError,OSError,KeyError,TypeError,OverflowError) as error:
        print('rejected: '+str(error));return 3
    print('accepted');return 0

def main(args):
    capture=args.capture.resolve();out=args.output.resolve();require(not out.exists(),'fresh negative directory');out.mkdir()
    m=subject();cases={r.name:r for r in m.recipes()};commands=[]
    files=[Path(__file__),HERE/'collector.py',HERE/'collector_oracle.c',capture/'evidence.json',capture/'recipes.json',capture/'collector-normal',capture/'collector-sanitized']
    state=lambda:[{'path':str(p),'bytes':p.stat().st_size,'sha256':sha(p)} for p in files]
    before=state();env={k:v for k,v in os.environ.items() if k in {'PATH','HOME','TMPDIR','TMP','TEMP','SDKROOT','DEVELOPER_DIR','MACOSX_DEPLOYMENT_TARGET'}}|{'LC_ALL':'C','LANG':'C','TZ':'UTC','PYTHONDONTWRITEBYTECODE':'1'}
    def run(label,argv,code,text,mode):
        require(state()==before,'frozen source/capture before check')
        effective=env|({'ASAN_OPTIONS':'halt_on_error=1:detect_leaks=0','UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'} if mode=='sanitized' else {})
        p=subprocess.run(argv,cwd=out,env=effective,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=30)
        stdout=out/(label+'.stdout');stderr=out/(label+'.stderr');stdout.write_text(p.stdout);stderr.write_text(p.stderr)
        commands.append({'label':label,'mode':mode,'command':argv,'environment':effective,'cwd':str(out),'exit_code':p.returncode,'expected_exit_code':code,'required_diagnostic':text,'stdout':stdout.name,'stdout_sha256':sha(stdout),'stderr':stderr.name,'stderr_sha256':sha(stderr)})
        save(out/'commands.json',commands)
        require(p.returncode==code and text in p.stdout+p.stderr,f'{label}: wrong outcome {p.returncode}: {p.stdout}{p.stderr}')
        require('ERROR: AddressSanitizer' not in p.stderr and 'runtime error:' not in p.stderr,'invalid harness access in rejection')
        if mode in ('normal','sanitized') and code:require('"type":"public"' not in p.stdout,'malformed packet reached API execution')
        require(state()==before,'frozen source/capture after check')
    base=cases['width-4'].packet();packets=[]
    def packet(name,data,reason):
        path=out/(name+'.packet');path.write_bytes(data);packets.append((name,path,reason))
    def scalar(index,value):
        b=bytearray(base);struct.pack_into('<I',b,72+4*index,value);return bytes(b)
    packet('short-header',base[:100],'packet fixed header')
    packet('wrong-magic',b'X'+base[1:],'packet magic')
    packet('empty-id',base[:8]+b'\0'*64+base[72:],'packet id extent')
    packet('unterminated-id',base[:8]+b'a'*64+base[72:],'packet id extent')
    packet('quoted-id',base[:8]+b'"'+base[9:],'packet id alphabet')
    packet('nonzero-id-padding',base[:8]+b'a\0x'+b'\0'*61+base[72:],'packet id padding')
    for name,index,value,reason in [('source-cap',0,524289,'packet source/window domain'),('window-low',1,9,'packet source/window domain'),('window-high',1,21,'packet source/window domain'),('hash-low',2,5,'packet parser domain'),('hash-high',2,19,'packet parser domain'),('match-low',3,2,'packet parser domain'),('match-high',3,8,'packet parser domain'),('target-cap',4,131073,'packet parser domain'),('capacity-kind',5,2,'packet capacity kind/value'),('default-capacity-value',6,1,'packet capacity kind/value'),('reserved',7,1,'packet reserved field')]:packet(name,scalar(index,value),reason)
    b=bytearray(base);struct.pack_into('<II',b,92,1,0xffffffff);packet('capacity-over-bound',bytes(b),'packet capacity bound')
    packet('short-source',base[:-1],'packet truncated source');packet('extra-source',base+b'x','packet trailing bytes/read error')
    for mode in ('normal','sanitized'):
        exe=str(capture/('collector-'+mode))
        run(mode+'-positive',[exe,str(capture/'packets/width-4.packet')],0,'"type":"corroboration"',mode)
        for name,path,reason in packets:run(mode+'-'+name,[exe,str(path)],1,reason,mode)
    mutations=[]
    def mutate(name,cid,change,reason):
        rows=[json.loads(x) for x in (capture/('normal-'+cid+'.stdout')).read_text().splitlines()];change(rows)
        path=out/(name+'.jsonl');path.write_text(''.join(json.dumps(r,separators=(',',':'))+'\n' for r in rows));mutations.append((name,cid,path,reason))
    cid='width-4'
    mutate('missing-record',cid,lambda r:r.pop(),'missing corroboration')
    mutate('extra-record',cid,lambda r:r.append(r[-1]),'unexpected records')
    mutate('metadata-unknown',cid,lambda r:r[0].__setitem__('extra',0),'metadata schema')
    mutate('metadata-bool',cid,lambda r:r[0].__setitem__('direct_next_to_update',True),'boolean impersonates integer')
    mutate('source-hash',cid,lambda r:r[1].__setitem__('source_fnv','0'*16),'source identity')
    mutate('source-float',cid,lambda r:r[1].__setitem__('source_bytes',1024.0),'nonintegral/unrecognized scalar')
    mutate('requested-detached',cid,lambda r:r[1]['requested'].__setitem__('hash_log',7),'exact requested parameters')
    mutate('capacity-detached',cid,lambda r:r[1].__setitem__('capacity',0),'output capacity')
    mutate('error-partial-rows','tiny-1',lambda r:r[1].__setitem__('rows',[]),'error must not serialize')
    mutate('empty-fabricated','tiny-0',lambda r:r[1].__setitem__('rows',[[0,0,0,0]]),'empty public result')
    mutate('direct-mismatch-bool',cid,lambda r:r[2].__setitem__('direct_mismatches',False),'boolean impersonates integer')
    mutate('wire-mismatch-bool',cid,lambda r:r[2].__setitem__('public_wire_mismatches',False),'boolean impersonates integer')
    mutate('wire-global-float',cid,lambda r:r[2].__setitem__('direct_global_wire_mismatches',0.0),'nonintegral/unrecognized scalar')
    mutate('long-type-bool',cid,lambda r:r[2]['blocks'][0].__setitem__('long_type',False),'boolean impersonates integer')
    mutate('long-position-bool',cid,lambda r:r[2]['blocks'][0].__setitem__('long_position',False),'boolean impersonates integer')
    mutate('long-marker','long-literal',lambda r:r[2]['blocks'][0].__setitem__('long_type',0),'exact expanded long marker')
    mutate('block-size',cid,lambda r:r[2]['blocks'][0].__setitem__('bytes',1023),'block partition')
    mutate('block-counter',cid,lambda r:r[2]['blocks'][0].__setitem__('sequence_count',0),'direct delimiter bound')
    mutate('match-ll',cid,lambda r:r[2]['direct_rows'][0].__setitem__(0,2048),'direct sequence extents')
    mutate('match-distance',cid,lambda r:r[2]['direct_rows'][0].__setitem__(2,0),'direct independent offset')
    mutate('match-intent',cid,lambda r:r[2]['direct_rows'][0].__setitem__(3,0),'direct sequence extents')
    mutate('public-distance',cid,lambda r:r[1]['rows'][0].__setitem__(2,2048),'reported direct mismatch counts')
    mutate('delimiter-rep',cid,lambda r:r[1]['rows'][-1].__setitem__(3,1),'reported direct mismatch counts')
    mutate('parser-third',cid,lambda r:r[2]['blocks'][0]['parser_after'].__setitem__(2,4),'Fast keeps third parser cell')
    mutate('candidate-continuity','history-period',lambda r:r[2]['blocks'][1]['parser_before'].__setitem__(0,1),'persistent candidate state')
    mutate('table-old',cid,lambda r:r[2]['blocks'][0]['changes'][0].__setitem__(1,1),'ordered complete delta')
    mutate('table-duplicate',cid,lambda r:r[2]['blocks'][0]['changes'].append(r[2]['blocks'][0]['changes'][0]),'ordered complete delta')
    mutate('table-key-tail',cid,lambda r:r[2]['blocks'][0]['changes'][0].__setitem__(2,1025),'changed key complete')
    mutate('table-digest',cid,lambda r:r[2]['blocks'][0].__setitem__('table_after_fnv','0'*16),'full table after fingerprint')
    mutate('collector-table-digest',cid,lambda r:r[2].__setitem__('collector_table_fnv','0'*16),'collector full table hash')
    mutate('table-equality',cid,lambda r:r[2].__setitem__('tables_equal',False),'table equality report')
    mutate('paired-padding-detachment',cid,lambda r:r[4]['collector_parser_after'].__setitem__(2,9),'complete initialized-padding independence')
    upstream=args.upstream.resolve();manifest=args.source_manifest.resolve();repo=next(p for p in manifest.parents if (p/'.git').exists())
    overlap=[('helper',HERE/'negative-must-not-exist'),('upstream',upstream/'negative-must-not-exist'),('collection',manifest.parent/'negative-must-not-exist'),('repo',repo/'negative-must-not-exist'),('manifest-descendant',manifest/'negative-must-not-exist')]
    for optimized in (False,True):
        mode='python-optimized' if optimized else 'python-normal';prefix=[sys.executable,*(['-O'] if optimized else [])]
        childprefix=prefix+[str(Path(__file__)),'--child']
        run(mode+'-positive',childprefix+['--case',cid,'--record',str(capture/('normal-'+cid+'.stdout'))],0,'accepted',mode)
        for name,caseid,path,reason in mutations:run(mode+'-'+name,childprefix+['--case',caseid,'--record',str(path)],3,'rejected: '+reason,mode)
        for name,path in overlap:
            require(not path.exists(),'negative overlap probe preexists')
            run(mode+'-overlap-'+name,prefix+[str(HERE/'collector.py'),'--upstream',str(upstream),'--source-manifest',str(manifest),'--output',str(path)],1,'capture/protected source path overlap',mode)
            require(not path.exists(),'protected path was created before rejection')
    require(state()==before,'final frozen sources/capture')
    summary={'status':'passed','commands':len(commands),'positive_controls':sum(c['expected_exit_code']==0 for c in commands),'expected_rejections':sum(c['expected_exit_code']!=0 for c in commands),'packet_negatives_per_build':len(packets),'record_negatives_per_python_mode':len(mutations),'path_overlap_negatives_per_python_mode':len(overlap),'source_before':before,'source_after':state(),'commands_sha256':sha(out/'commands.json')}
    save(out/'summary.json',summary);print(json.dumps({k:v for k,v in summary.items() if k not in ('source_before','source_after')},indent=2))

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--child',action='store_true');p.add_argument('--case');p.add_argument('--record',type=Path)
    p.add_argument('--capture',type=Path);p.add_argument('--output',type=Path);p.add_argument('--upstream',type=Path);p.add_argument('--source-manifest',type=Path)
    args=p.parse_args()
    if args.child:raise SystemExit(child(args))
    main(args)
