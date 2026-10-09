#!/usr/bin/env python3
"""Bounded unchanged Fast collector capture and independent candidate replay."""
from pathlib import Path
from dataclasses import dataclass, replace
import argparse, datetime, hashlib, json, os, re, shutil, struct, subprocess, traceback
HERE=Path(__file__).resolve().parent
REVISION='f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_SOURCE=512<<10
MAX_RECORD_BYTES=64<<20

def require(ok,reason):
    if not ok:raise ValueError(reason)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def fnv(data):
    n=0xcbf29ce484222325
    for b in data:n=((n^b)*0x100000001b3)&0xffffffffffffffff
    return f'{n:016x}'
def exact(a,b):return json.dumps(a,sort_keys=True,allow_nan=False)==json.dumps(b,sort_keys=True,allow_nan=False)
def write_json(path,value):path.write_text(json.dumps(value,sort_keys=True,indent=2)+'\n')
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat()
def decode_json(text):
    def fields(pairs):
        result={}
        for k,v in pairs:
            require(k not in result,f'duplicate JSON field {k}');result[k]=v
        return result
    def bad(s):raise ValueError(f'nonfinite scalar {s}')
    return json.loads(text,object_pairs_hook=fields,parse_constant=bad)
def integer(n,lo,hi,reason):require(type(n) is int and lo<=n<=hi,reason);return n
def keys(value,expected,reason):require(type(value) is dict and set(value)==set(expected.split()),reason)
def vector(value,count,lo,hi,reason):
    require(type(value) is list and len(value)==count,reason)
    for v in value:integer(v,lo,hi,reason)
    return value

def bucket(key,log):
    width=len(key);word=int.from_bytes(key,'little')
    require(width in (4,5,6,7) and 6<=log<=18,'hash domain')
    if width==4:return ((word*2654435761)&0xffffffff)>>(32-log)
    return (((word<<(64-width*8))*{5:889523592379,6:227718039650203,7:58295818150454627}[width])&0xffffffffffffffff)>>(64-log)
def noise(n,seed=0x31415926):
    out=bytearray()
    for _ in range(n):
        seed=(seed*1664525+1013904223)&0xffffffff;out.append(((seed>>24)^(seed>>13))&255)
    return bytes(out)
def debruijn():
    a=[0]*64;out=bytearray()
    def rec(t,p):
        if t>4:
            if 4%p==0:out.extend(a[1:p+1])
        else:
            a[t]=a[t-p];rec(t+1,p)
            for v in range(a[t-p]+1,16):a[t]=v;rec(t+1,t)
    rec(1,1);require(len(out)==65536,'de Bruijn length');return bytes(out)

@dataclass(frozen=True)
class Recipe:
    name:str
    family:str
    source:bytes
    window:int=10
    hash_log:int=6
    minimum:int=4
    target:int=0
    capacity:int|None=None
    derived_from:str|None=None
    def packet(self):
        require(re.fullmatch('[a-z0-9_-]{1,63}',self.name) is not None,'recipe id')
        require(len(self.source)<=MAX_SOURCE and 10<=self.window<=20 and 6<=self.hash_log<=18,'recipe source/table')
        require(3<=self.minimum<=7 and 0<=self.target<=131072,'recipe parser')
        bound=len(self.source)//3+1+len(self.source)//1024+1
        require(self.capacity is None or type(self.capacity) is int and 0<=self.capacity<=bound,'recipe capacity')
        scalars=(len(self.source),self.window,self.hash_log,self.minimum,self.target,int(self.capacity is not None),self.capacity or 0,0)
        return b'ZCOLL01\0'+self.name.encode().ljust(64,b'\0')+struct.pack('<8I',*scalars)+self.source
    def metadata(self):
        data=self.packet()
        return {'id':self.name,'family':self.family,'source_bytes':len(self.source),'source_sha256':hashlib.sha256(self.source).hexdigest(),'packet_bytes':len(data),'packet_sha256':hashlib.sha256(data).hexdigest(),'window_log':self.window,'hash_log':self.hash_log,'min_match':self.minimum,'target':self.target,'capacity':self.capacity,'derived_from':self.derived_from}

def recipes():
    result=[Recipe(f'tiny-{n}','tiny',bytes(range(n))) for n in range(17)]
    for n in (16,1024):
        for large in (False,True):result.append(Recipe(f'adjust-{n}-{int(large)}','adjustment',(b'aBcDeF0123456789!'*((n+15)//16))[:n],window=20 if large else 10,hash_log=18 if large else 6))
    for m in range(3,8):result.append(Recipe(f'width-{m}','width',noise(128)*8,minimum=m))
    for t in (3,4,131072):result.append(Recipe(f'target-{t}','target',noise(256)*4,target=t))
    result += [Recipe('long-uniform','long',b'Q'*65700,window=17,hash_log=16),
               Recipe('long-literal','long',debruijn()+debruijn()[:2048],window=17,hash_log=18),
               Recipe('long-match-65540','long',b'X'*65540,window=17,hash_log=16),
               Recipe('long-match-65541','long',b'X'*65541,window=17,hash_log=16),
               Recipe('history-copy-three','history',noise(1024)*3),
               Recipe('history-period','history',(noise(96)*32)),
               Recipe('history-expiry','history',noise(1024)+noise(1024,7)+noise(1024)),
               Recipe('history-large','history',noise(65536)*4,window=18,hash_log=18),
               Recipe('history-alternating','history',(noise(64)+noise(64,8))*24),
               Recipe('tiny-final-1','tiny-final',noise(1024)+b'x'),
               Recipe('tiny-final-6','tiny-final',noise(1024)+b'abcdef')]
    require(len(result)==40 and len({r.name for r in result})==40,'frozen base recipe cardinality')
    return result

def resolve(rep,off,ll):
    a,b,c=rep;require(all(x>0 for x in rep) and off>0,'positive repeats/offBase')
    if off>3:return off-3,[off-3,a,b]
    if ll and off==1:return a,[a,b,c]
    if ll and off==2 or not ll and off==1:return b,[b,a,c]
    distance=a-1 if not ll and off==3 else c
    require(distance>0,'repeat-minus-one positive');return distance,[distance,a,b]
def params(p):
    keys(p,'window_log hash_log chain_log search_log min_match target strategy','parameter fields')
    for name,lo,hi in (('window_log',10,20),('hash_log',6,18),('chain_log',6,20),('search_log',1,20),('min_match',3,7),('target',0,131072),('strategy',1,1)):integer(p[name],lo,hi,'applied '+name)
def table_hash(table):return fnv(struct.pack('<'+str(len(table))+'I',*table))

def validate(case,path):
    require(path.stat().st_size<=MAX_RECORD_BYTES,'record file cap')
    records=[decode_json(x) for x in path.read_text().splitlines()]
    def scalar_types(value,field=None):
        if type(value) is dict:
            for key,item in value.items():scalar_types(item,key)
        elif type(value) is list:
            for item in value:scalar_types(item)
        elif type(value) is bool:require(field in {'little_endian','parser_first_two_equal','tables_equal'},'boolean impersonates integer')
        else:require(value is None or type(value) in (int,str),'nonintegral/unrecognized scalar')
    scalar_types(records)
    require(records,'nonempty records');meta=records.pop(0)
    keys(meta,'type revision version size_t_bytes sequence_bytes little_endian source_cap block_cap suffix_bytes direct_next_to_update','metadata schema')
    require(meta['type']=='metadata' and meta['revision']==REVISION and meta['version']==10507,'pinned metadata')
    require(type(meta['little_endian']) is bool and meta['size_t_bytes'] in (4,8),'ABI metadata')
    require(type(meta['sequence_bytes']) is int and meta['sequence_bytes']==16,'pinned sequence ABI')
    require(meta['source_cap']==MAX_SOURCE and meta['block_cap']==131072 and meta['suffix_bytes']==262656 and meta['direct_next_to_update']==2,'setup metadata')
    observations=[]
    for padding in (106,195):
        require(records,'missing public observation');public=records.pop(0)
        keys(public,'type id padding source_bytes source_fnv bound capacity requested error error_name rows','public schema')
        require(public['type']=='public' and public['id']==case.name and public['padding']==padding,'public identity')
        source=case.source;n=len(source);bound=n//3+1+n//1024+1
        require(public['source_bytes']==n and public['source_fnv']==fnv(source),'source identity')
        require(public['bound']==bound and public['capacity']==(bound if case.capacity is None else case.capacity),'output capacity')
        require(exact(public['requested'],{'window_log':case.window,'hash_log':case.hash_log,'chain_log':6,'search_log':1,'min_match':case.minimum,'target':case.target,'strategy':1}),'exact requested parameters')
        error=integer(public['error'],0,255,'error code');require(type(public['error_name']) is str,'error name type')
        if error:
            require(public['rows'] is None,'error must not serialize partially produced rows');observations.append((public,None));continue
        require(type(public['rows']) is list and len(public['rows'])<=public['capacity'],'successful row capacity')
        rows=public['rows']
        for row in rows:vector(row,4,0,(1<<32)-1,'public row')
        if not n:
            require(rows==[],'empty public result observed without private state');observations.append((public,None));continue
        require(records,'missing corroboration');record=records.pop(0)
        keys(record,'type id padding applied max_block_parameter effective_block_bytes direct_mismatches public_wire_mismatches direct_global_wire_mismatches public_wire_after parser_after collector_parser_after parser_first_two_equal tables_equal collector_final_table collector_table_fnv direct_rows blocks','corroboration schema')
        require(record['type']=='corroboration' and record['id']==case.name and record['padding']==padding,'corroboration identity')
        p=record['applied'];params(p);width=max(4,p['min_match']);window=1<<p['window_log'];block_max=integer(record['effective_block_bytes'],1,min(window,131072),'effective block')
        require(record['max_block_parameter']==131072,'applied max block parameter')
        require(type(record['direct_rows']) is list and len(record['direct_rows'])==len(rows),'direct/public row extent')
        for row in record['direct_rows']:vector(row,4,0,(1<<32)-1,'direct row')
        for field in ('direct_mismatches','public_wire_mismatches','direct_global_wire_mismatches'):
            integer(record[field],0,len(rows),field)
        require(type(record['blocks']) is list and len(record['blocks'])==(n+block_max-1)//block_max,'complete block schedule')
        table=[0]*(1<<p['hash_log']);parser=[1,4,8];global_rep=[1,4,8];cursor=0;row_index=0;decoded=bytearray();global_mismatch=0;direct_mismatch=0
        for block in record['blocks']:
            keys(block,'start bytes first_row sequence_count trailing long_type long_position parser_before parser_after wire_after table_before_fnv table_after_fnv changes','block schema')
            start=cursor;size=min(block_max,n-start);floor=max(0,start-window)
            require(size>=7 and block['start']==start and block['bytes']==size and block['first_row']==row_index,'block partition')
            count=integer(block['sequence_count'],0,size//3+1,'sequence count');trailing=integer(block['trailing'],0,size,'trailing')
            require(exact(block['parser_before'],parser),'persistent candidate state')
            local=parser.copy();longs=[]
            for sequence_index in range(count):
                require(row_index<len(rows),'direct sequence count');ll,ml,distance,off=record['direct_rows'][row_index]
                require(ml>=3 and off>0 and ll<=start+size-cursor,'direct sequence extents')
                if ll>=65536:longs.append((1,sequence_index))
                if ml-3>=65536:longs.append((2,sequence_index))
                decoded.extend(source[cursor:cursor+ll]);cursor+=ll
                expected,local=resolve(local,off,ll);global_distance,global_rep=resolve(global_rep,off,ll)
                global_mismatch+=global_distance!=distance
                require(distance==expected and 0<distance<=min(cursor,window) and cursor-distance>=floor,'direct independent offset resolution')
                require(ml<=start+size-cursor,'direct match bound')
                for _ in range(ml):decoded.append(decoded[-distance])
                require(decoded[cursor:cursor+ml]==source[cursor:cursor+ml],'direct byte replay');cursor+=ml
                direct_mismatch+=rows[row_index]!=[ll,ml,distance,off if off<=3 else 0];row_index+=1
            require(len(longs)<=1,'single direct long length');expected_long=longs[0] if longs else (0,0)
            integer(block['long_type'],0,2,'long length type')
            integer(block['long_position'],0,max(0,count-1),'long length position')
            require((block['long_type'],block['long_position'])==expected_long,'exact expanded long marker')
            require(trailing==start+size-cursor and row_index<len(rows),'direct delimiter bound')
            require(record['direct_rows'][row_index]==[trailing,0,0,0],'direct delimiter fields')
            direct_mismatch+=rows[row_index]!=[trailing,0,0,0]
            decoded.extend(source[cursor:cursor+trailing]);cursor+=trailing;row_index+=1
            require(exact(block['wire_after'],local),'candidate full wire repeats')
            parser=[local[0],local[1],parser[2]];require(exact(block['parser_after'],parser),'Fast keeps third parser cell')
            require(block['table_before_fnv']==table_hash(table),'full table before fingerprint')
            require(type(block['changes']) is list and len(block['changes'])<=size,'bounded per-block changes')
            previous=-1
            for change in block['changes']:
                slot,old,new=vector(change,3,0,(1<<32)-1,'table change')
                require(previous<slot<len(table) and old==table[slot] and old!=new,'ordered complete delta');previous=slot
                require(new>=2 and start<=new-2<=start+size-width,'changed key complete in current block')
                require(bucket(source[new-2:new-2+width],p['hash_log'])==slot,'changed key independent hash');table[slot]=new
            require(block['table_after_fnv']==table_hash(table),'full table after fingerprint')
        require(cursor==n and row_index==len(rows) and decoded==source,'complete direct replay')
        require(exact(record['parser_after'],parser),'direct final parser state')
        require(record['direct_mismatches']==direct_mismatch and record['direct_global_wire_mismatches']==global_mismatch,'reported direct mismatch counts')
        final=[0]*len(table);previous=-1
        require(type(record['collector_final_table']) is list,'collector final entries type')
        for entry in record['collector_final_table']:
            slot,value=vector(entry,2,0,(1<<32)-1,'collector table entry')
            require(previous<slot<len(final) and value>=2 and value-2<=n-width,'collector complete selected key');previous=slot
            require(bucket(source[value-2:value-2+width],p['hash_log'])==slot,'collector final key hash');final[slot]=value
        require(record['collector_table_fnv']==table_hash(final),'collector full table hash')
        require(type(record['tables_equal']) is bool and record['tables_equal']==(table==final),'table equality report')
        vector(record['collector_parser_after'],3,1,(1<<32)-1,'collector parser repeat triple')
        require(type(record['parser_first_two_equal']) is bool and record['parser_first_two_equal']==(parser[:2]==record['collector_parser_after'][:2]),'parser first-two equality report')
        # Independent replay of the PUBLIC offsets, retaining rather than fixing
        # any selector disagreement with the continuous full wire repeat triple.
        decoded=bytearray();cursor=0;start=0;wire=[1,4,8];wire_mismatch=0;delimiters=0
        for ll,ml,distance,rep in rows:
            require(ll<=n-cursor,'public literal extent');decoded.extend(source[cursor:cursor+ll]);cursor+=ll
            if not ml and not distance:
                require(rep==0 and cursor-start==min(block_max,n-start),'public delimiter contract');start=cursor;delimiters+=1;continue
            require(ml>=3 and 0<distance<=min(cursor,window) and cursor-distance>=max(0,start-window) and rep<=3,'public match contract')
            resolved,wire=resolve(wire,rep or distance+3,ll);wire_mismatch+=resolved!=distance
            require(ml<=n-cursor,'public match extent')
            for _ in range(ml):decoded.append(decoded[-distance])
            require(decoded[cursor:cursor+ml]==source[cursor:cursor+ml],'public offset byte replay');cursor+=ml
        require(cursor==start==n and decoded==source and delimiters==len(record['blocks']),'public full coverage')
        require(exact(record['public_wire_after'],wire) and record['public_wire_mismatches']==wire_mismatch,'public wire mismatch report')
        observations.append((public,record))
    require(not records,'unexpected records after both padding observations')
    def normalized(pair):return tuple(None if item is None else {k:v for k,v in item.items() if k!='padding'} for item in pair)
    require(exact(normalized(observations[0]),normalized(observations[1])),'complete initialized-padding independence')
    public,record=observations[0]
    return {'id':case.name,'observations':2,'error':public['error'],'error_name':public['error_name'],'rows':None if public['rows'] is None else len(public['rows']),'blocks':None if record is None else len(record['blocks']),'applied':None if record is None else record['applied'],'direct_mismatches':None if record is None else record['direct_mismatches'],'public_wire_mismatches':None if record is None else record['public_wire_mismatches'],'direct_global_wire_mismatches':None if record is None else record['direct_global_wire_mismatches'],'tables_equal':None if record is None else record['tables_equal'],'parser_first_two_equal':None if record is None else record['parser_first_two_equal']}

def run(args):
    upstream=args.upstream.resolve();output=args.output.resolve()
    source_manifest=args.source_manifest.resolve()
    repo=next((parent for parent in source_manifest.parents if (parent/'.git').exists()),None)
    require(repo is not None,'source manifest must identify its protected repository root')
    protected=(HERE,upstream,source_manifest,source_manifest.parent,repo)
    require(not output.exists(),'fresh capture directory')
    for root in protected:
        require(not output.is_relative_to(root) and not root.is_relative_to(output),'capture/protected source path overlap')
    output.mkdir(parents=True)
    env={k:v for k,v in os.environ.items() if k in {'PATH','HOME','TMPDIR','TMP','TEMP','SYSTEMROOT','SDKROOT','MACOSX_DEPLOYMENT_TARGET','DEVELOPER_DIR'}}|{'LC_ALL':'C','LANG':'C','TZ':'UTC'}
    cc=shutil.which(args.cc,path=env.get('PATH'));require(cc is not None,'compiler unavailable');cc=str(Path(cc).resolve())
    upstream_manifest=decode_json(args.source_manifest.read_text())
    require(upstream_manifest['upstream_revision']==REVISION,'source manifest revision')
    for name,digest in upstream_manifest['source_sha256'].items():require(sha(upstream/name)==digest,'pinned source '+name)
    cases=recipes();packets=output/'packets';packets.mkdir()
    for case in cases:(packets/(case.name+'.packet')).write_bytes(case.packet())
    write_json(output/'recipes.json',{'schema_version':1,'phase':'before any C execution','generator_sha256':sha(Path(__file__)),'case_count':len(cases),'cases':[case.metadata() for case in cases],'capacity_plan':['width-4','history-period'],'capacity_variants':['observed exact count','observed count minus one','zero'],'qualification':'Capacity fields only depend on first observed normal successful row counts. Source bytes remain independently frozen.'})
    # Complete pinned library source closure is captured in addition to the
    # specific reviewed source manifest and actual archive being linked.
    sources=[Path(__file__).resolve(),HERE/'collector_oracle.c',args.source_manifest.resolve(),upstream/'lib/libzstd.a',output/'recipes.json',*sorted(packets.iterdir()),*[p for p in sorted((upstream/'lib').rglob('*')) if p.is_file() and p.suffix in {'.c','.h','.S'}]]
    def state():return [{'path':str(p),'bytes':p.stat().st_size,'sha256':sha(p)} for p in sources]
    before=state();derived=[];derived_pins=[]
    evidence={'schema_version':1,'started_at':utc(),'status':'running','commands':[],'source_before':before,'environment':env,'compiler':cc,'compiler_sha256':sha(Path(cc)),'base_recipes':40,'capacity_variants':6}
    def save():write_json(output/'evidence.json',evidence)
    def command(label,argv,overrides=None):
        require(state()==before,'source/base packet changed before command')
        for item in derived_pins:require(sha(Path(item['path']))==item['sha256'],'derived packet changed')
        out=output/(label+'.stdout');err=output/(label+'.stderr');effective=env|(overrides or {});start=utc()
        with out.open('w') as stdout,err.open('w') as stderr:
            completed=subprocess.run(argv,cwd=output,env=effective,stdout=stdout,stderr=stderr,timeout=180,check=False)
        evidence['commands'].append({'label':label,'command':argv,'cwd':str(output),'environment':effective,'started_at':start,'finished_at':utc(),'exit_code':completed.returncode,'stdout':out.name,'stderr':err.name,'stdout_sha256':sha(out),'stderr_sha256':sha(err),'source_before_matches':True,'source_after_matches':state()==before,'derived_packets_checked':len(derived_pins)})
        save();require(completed.returncode==0,'command failed: '+label);require(state()==before,'source/base packet changed after command')
        for item in derived_pins:require(sha(Path(item['path']))==item['sha256'],'derived packet changed after command')
        return out
    save()
    try:
        require(command('reference-revision',['git','-C',str(upstream),'rev-parse','HEAD']).read_text().strip()==REVISION,'reference revision')
        require(command('reference-status',['git','-C',str(upstream),'status','--porcelain']).read_text()=='','reference clean')
        command('compiler-version',[cc,'--version']);results={}
        for mode in (('normal','sanitized') if args.sanitize else ('normal',)):
            executable=output/('collector-'+mode)
            cmd=[cc,'-std=c11','-Wall','-Wextra','-Werror','-I',str(upstream/'lib'),'-I',str(upstream/'lib/common'),'-I',str(upstream/'lib/compress')]
            if mode=='normal':cmd+=['-O2',str(HERE/'collector_oracle.c')]
            else:cmd+=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',str(HERE/'collector_oracle.c'),str(upstream/'lib/compress/zstd_compress.c'),str(upstream/'lib/compress/zstd_fast.c')]
            cmd +=[str(upstream/'lib/libzstd.a'),'-o',str(executable)];command(mode+'-compile',cmd)
            overrides={'ASAN_OPTIONS':'halt_on_error=1:detect_leaks=0','UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'} if mode=='sanitized' else {}
            rows=[]
            def capture(case):
                path=command(mode+'-'+case.name,[str(executable),str(packets/(case.name+'.packet'))],overrides)
                rows.append(validate(case,path))
                if mode=='sanitized':require(path.read_bytes()==(output/('normal-'+case.name+'.stdout')).read_bytes(),'normal/sanitized exact records')
            for case in cases:capture(case)
            if mode=='normal':
                for parent in ('width-4','history-period'):
                    original=next(r for r in cases if r.name==parent);observation=next(r for r in rows if r['id']==parent)
                    require(observation['error']==0 and observation['rows']>0,'capacity parent must actually succeed')
                    count=observation['rows']
                    for label,capacity in (('exact',count),('short',count-1),('zero',0)):
                        variant=replace(original,name=parent+'-capacity-'+label,family='capacity',capacity=capacity,derived_from=parent)
                        path=packets/(variant.name+'.packet');path.write_bytes(variant.packet());derived.append(variant)
                        derived_pins.append({'path':str(path),'sha256':sha(path)})
                require(len(derived)==6,'derived capacity count')
                write_json(output/'capacity-recipes.json',{'phase':'after normal baseline, before capacity or sanitizer calls','cases':[r.metadata() for r in derived],'packet_pins':derived_pins})
                derived_pins.append({'path':str(output/'capacity-recipes.json'),'sha256':sha(output/'capacity-recipes.json')})
                evidence['capacity_derivation']={'file':'capacity-recipes.json','sha256':sha(output/'capacity-recipes.json')};save()
            for case in derived:capture(case)
            require(len(rows)==46,'complete base and capacity cases')
            results[mode]={'case_count':len(rows),'observations':sum(r['observations'] for r in rows),'error_observations':2*sum(r['error']!=0 for r in rows),'successful_nonempty_observations':2*sum(r['blocks'] is not None for r in rows),'executable_sha256':sha(executable),'cases':rows}
            evidence['results']=results;save()
        evidence['source_after']=state();require(evidence['source_after']==before,'final frozen source')
        evidence['status']='passed';evidence['finished_at']=utc();evidence['instrumentation']={'helper_compress_c_fast_c_inline_headers':args.sanitize,'remaining_archive_instrumented':False,'leak_sanitizer':False}
        evidence['scope']='Collector candidate publication only; actual errors and semantic repeat disagreements remain observations, not ordinary frame guarantees or full upstream fuzz migration.'
        save();print(f'Validated 46 collector cases / 92 padding observations per mode at {output}')
    except BaseException:
        evidence['status']='failed';evidence['failure']=traceback.format_exc();save();raise

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream',required=True,type=Path);parser.add_argument('--source-manifest',required=True,type=Path)
    parser.add_argument('--output',required=True,type=Path);parser.add_argument('--cc',default='cc');parser.add_argument('--sanitize',action='store_true')
    run(parser.parse_args())
if __name__=='__main__':main()
