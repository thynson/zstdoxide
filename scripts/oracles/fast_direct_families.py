#!/usr/bin/env python3
"""Bounded direct Fast family recipes and independent replay; test oracle only."""
from pathlib import Path
from dataclasses import dataclass, replace
import argparse, datetime, hashlib, json, os, re, shutil, struct, subprocess, sys, traceback

HERE=Path(__file__).resolve().parent
REVISION='f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_SOURCE=2<<20
MAX_BLOCK=128<<10
MAX_CASES=192
MAX_SEEDS=1024
MAX_RECORDS=32<<20


def require(ok, reason):
    if not ok: raise ValueError(reason)


def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def fnv(data):
    result=0xcbf29ce484222325
    for byte in data: result=((result^byte)*0x100000001b3)&0xffffffffffffffff
    return f'{result:016x}'
def write_json(path,value): path.write_text(json.dumps(value,sort_keys=True,indent=2)+'\n')
def utc(): return datetime.datetime.now(datetime.timezone.utc).isoformat()
def exact(left,right): return json.dumps(left,sort_keys=True,allow_nan=False)==json.dumps(right,sort_keys=True,allow_nan=False)
def decode_json(line):
    def fields(pairs):
        result={}
        for key,value in pairs:
            require(key not in result,f'duplicate JSON field {key}')
            result[key]=value
        return result
    def bad(value): raise ValueError(f'nonfinite JSON scalar {value}')
    return json.loads(line,object_pairs_hook=fields,parse_constant=bad)


# Pinned zstd_compress_internal.h:903-920, modulo32/64 after multiply.
# Read exactly the selected logical key, independent of C's physical read width.
def bucket(key,hash_log):
    width=len(key); word=int.from_bytes(key,'little')
    require(width in (4,5,6,7) and 6<=hash_log<=18,'hash domain')
    if width==4: return ((word*2654435761)&0xffffffff)>>(32-hash_log)
    prime={5:889523592379,6:227718039650203,7:58295818150454627}[width]
    return (((word<<(64-8*width))*prime)&0xffffffffffffffff)>>(64-hash_log)


@dataclass(frozen=True)
class Recipe:
    name:str
    family:str
    source:bytes
    prefix:int=0
    window_log:int=10
    hash_log:int=6
    min_match:int=4
    target:int=0
    prefix_floor:int=0
    repeats:tuple=(1,4,8)
    seeds:tuple=()
    hypothesis:str='Boundary recipe; no branch assertion before observed output.'

    @property
    def block(self): return len(self.source)-self.prefix
    @property
    def width(self): return max(4,self.min_match)

    def packet(self):
        require(re.fullmatch('[a-z0-9_-]{1,63}',self.name) is not None,'recipe id')
        require(7<=self.block<=MAX_BLOCK and 0<=self.prefix_floor<=self.prefix,'recipe extents')
        require(len(self.source)<=MAX_SOURCE and 10<=self.window_log<=20,'source/window cap')
        require(self.block<=1<<self.window_log,'block window cap')
        require(6<=self.hash_log<=18 and 3<=self.min_match<=7 and 0<=self.target<=131072,'parameter cap')
        require(len(self.repeats)==3 and all(type(x) is int and 0<x<1<<32 for x in self.repeats),'positive repeat cells')
        require(len(self.seeds)<=MAX_SEEDS and all(type(x) is int and 0<=x and x+self.width<=self.prefix for x in self.seeds),'complete prior seed key')
        scalars=(self.prefix,self.block,self.window_log,self.hash_log,self.min_match,self.target,self.prefix_floor,*self.repeats,len(self.seeds),0)
        return b'ZFAST02\0'+self.name.encode().ljust(64,b'\0')+struct.pack('<12I',*scalars)+b''.join(struct.pack('<I',x) for x in self.seeds)+self.source

    def metadata(self):
        packet=self.packet()
        return {'id':self.name,'family':self.family,'source_bytes':len(self.source),'source_sha256':hashlib.sha256(self.source).hexdigest(),'packet_bytes':len(packet),'packet_sha256':hashlib.sha256(packet).hexdigest(),'prefix':self.prefix,'block':self.block,'window_log':self.window_log,'hash_log':self.hash_log,'min_match':self.min_match,'key_bytes':self.width,'target':self.target,'prefix_floor':self.prefix_floor,'initial_repeats':self.repeats,'ordered_seeds':self.seeds,'hypothesis':self.hypothesis}


def noise(size,seed=0x31415926):
    out=bytearray()
    for _ in range(size):
        seed=(seed*1664525+1013904223)&0xffffffff
        out.append(((seed>>24)^(seed>>13))&255)
    return bytes(out)


def debruijn():
    alphabet=16;order=4;work=[0]*(alphabet*order);out=bytearray()
    def visit(t,period):
        if t>order:
            if order%period==0:out.extend(work[1:period+1])
        else:
            work[t]=work[t-period];visit(t+1,period)
            for symbol in range(work[t-period]+1,alphabet):
                work[t]=symbol;visit(t+1,t)
    visit(1,1)
    require(len(out)==65536,'independent de Bruijn length')
    return bytes(out)


def overlapping_collision():
    # Fixed bounded enumeration; retain actual selected bytes in recipe metadata.
    for index in range(65536):
        value=(0x3ab1290705+index*0x9e3779b9)&((1<<40)-1)
        five=value.to_bytes(5,'little')
        if five[:4]!=five[1:] and bucket(five[:4],6)==bucket(five[1:],6):return five
    raise ValueError('no bounded overlapping collision candidate')


def recipes():
    result=[]
    def add(recipe): result.append(recipe)
    width_sources={
        'period':b'0123456789ABCDEF'*16,
        'wide-split':b''.join(b'abcd'+bytes([17+i,37+i,83+i,121+i])+b'01234567' for i in range(16)),
        'asymmetric':noise(192)+noise(192)[32:80]+noise(15,7),
    }
    for label,source in width_sources.items():
        for width in (3,4,5,6,7):
            for window in (18,19):
                add(Recipe(f'width-{label}-m{width}-w{window}','width-dispatch',source,window_log=window,min_match=width,
                           hypothesis='Compare requested3/4 equivalence and cmov/branch window18/19 with equal effective bounds.'))
    for hlog in (12,16,18):
        for width in (4,5,6,7):
            add(Recipe(f'table-h{hlog}-m{width}','conventional-table',noise(512)+noise(512)[17:193],window_log=20,hash_log=hlog,min_match=width))
    for size in (*range(7,17),23,24,25,31,32,33):
        add(Recipe(f'tail-{size}-t0','short-tail',noise(size)))
    for size in (12,16,24,33):
        for target in (3,4,131072):add(Recipe(f'tail-{size}-t{target}','short-tail',noise(size),target=target))
    unique=debruijn()
    for target in (0,1,2,3,4,5,131072):
        add(Recipe(f'accel-no-match-t{target}','acceleration',unique[97:609],target=target))
    for position in (127,128,129,255,256,257):
        for target in (0,3,4):
            source=bytearray(unique[97:609]);source[position:position+8]=source[16:24]
            add(Recipe(f'accel-plant-p{position}-t{target}','acceleration',bytes(source),target=target,
                       hypothesis='Planted repeat around restart-relative growth region; exact sampled outcome remains unobserved.'))
    five=overlapping_collision();prefix=bytearray(noise(64));prefix[:4]=five[1:]
    block=bytearray(noise(96,8));block[:5]=five
    add(Recipe('collision-adjacent','collision',bytes(prefix+block),prefix=64,repeats=(257,263,269),seeds=(0,),hypothesis='Adjacent candidate shares first probe bucket; lookup observes prior write.'))
    control=bytearray(block)
    for byte in range(256):
        control[0]=byte
        if bucket(control[:4],6)!=bucket(control[1:5],6):break
    else:raise ValueError('noncolliding adjacent control unavailable')
    add(Recipe('collision-adjacent-control','collision',bytes(prefix+control),prefix=64,repeats=(257,263,269),seeds=(0,),hypothesis='Adjacent match control differs only in first byte, avoiding the bucket collision.'))
    first=bytearray(block);first[:4]=prefix[:4]
    add(Recipe('collision-first-probe','collision',bytes(prefix+first),prefix=64,repeats=(257,263,269),seeds=(0,),hypothesis='First matching prefix candidate must remain observable before replacing its hash slot.'))
    for target in (3,4):
        second=bytearray(noise(96,91));second[1:5]=prefix[:4];second[5]=prefix[4]^0xff
        # Ensure the adjacent lookup is not destroyed by the first current key.
        for byte in range(256):
            second[0]=byte
            if bucket(second[:4],6)!=bucket(second[1:5],6):break
        else:raise ValueError('second-probe control unavailable')
        add(Recipe(f'second-probe-step{target+1}','collision',bytes(prefix+second),prefix=64,target=target,repeats=(257,263,269),seeds=(0,),hypothesis='Second-position four-byte match contrasts conditional next-key write for step4/5.'))
    # End soon after the second-position match to isolate the conditional
    # precomputed next-key store, without subsequent probes overwriting it.
    for attempt in range(64):
        short=bytearray(noise(15,100+attempt));short[1:5]=prefix[:4];short[5]=prefix[4]^0xff
        if len({bucket(short[i:i+4],6) for i in (0,1,3,4)})==4:break
    else:raise ValueError('isolated conditional-store keys unavailable')
    for target in (3,4):
        add(Recipe(f'conditional-store-step{target+1}','conditional-store',bytes(prefix+short),prefix=64,target=target,repeats=(257,263,269),seeds=(0,),hypothesis='Short cleanup isolates the step4 next-key store; compare final bucket against step5 absence.'))
    # Match-extension hypotheses with exact preceding equality lengths.
    for preceding in (0,1,3,7):
        prefix=bytearray(noise(128,901));block=bytearray(noise(96,333))
        block[8-preceding:24]=prefix[32-preceding:48]
        block[7-preceding]=prefix[31-preceding]^0xff
        for mode in ('offset','repeat'):
            reps=(104,19,31) if mode=='repeat' else (257,263,269)
            add(Recipe(f'extension-{mode}-{preceding}','extension',bytes(prefix+block),prefix=128,repeats=reps,seeds=(32,),hypothesis='Observe explicit/repeat preceding extension; source hypothesis alone is not branch evidence.'))
    # Multiple copied pieces provide opportunities for immediate zero-LL loops
    # and for zero/one/two new ordinary offsets after repeat invalidation.
    a=b'ABCDEFGHIJKLMNO!';b=b'qrstuvwxyz01234?'
    prefix=a+b+noise(64,55)
    for label,block in [('none',noise(96,44)),('one',a*4+noise(32,88)),('alternating',a+b+a+b+a+b),('two',a*2+b*2+noise(32,17)),('loop',(a+b)*2+prefix[64:96]*2)]:
        for saved in ('available','disabled'):
            reps=(32,16,8) if saved=='available' else (257,263,269)
            add(Recipe(f'repeats-{label}-{saved}','repeat-transition',prefix+block,prefix=len(prefix),repeats=reps,seeds=(0,16),hypothesis='Classify actual explicit/zero-LL repeated sequences and restored parser cells; no intended-path claim until measured.'))
    add(Recipe('repeats-single-disabled','repeat-transition',prefix+a+noise(80,123),prefix=len(prefix),repeats=(257,263,269),seeds=(0,16),hypothesis='One ordinary offset then distinct cleanup data should retain the disabled saved-first as second.'))
    add(Recipe('repeats-duplicate','repeat-transition',b'abcd'*48,prefix=32,repeats=(4,4,8),seeds=(0,4)))
    # One-kibibyte window with prefix1500/block64 gives fixed end-bound540.
    for shift in (-1,0,1):
        prefix=bytearray(noise(1500,83));block=bytearray(noise(64,79));position=540+shift
        block[:16]=prefix[position:position+16]
        add(Recipe(f'low-bound-{shift+1}','low-bound',bytes(prefix+block),prefix=1500,repeats=(4096,8192,16384),seeds=(position,),hypothesis='Seed around block-end-derived ordinary lower bound540; exact rejection/equality remains observed-only.'))
    for distance in (1024,1025):
        prefix=bytearray(noise(1500,83));block=bytearray(noise(64,79))
        for i in range(3,32):block[i]=prefix[1500+i-distance]
        add(Recipe(f'rep-bound-{distance}','low-bound',bytes(prefix+block),prefix=1500,repeats=(distance,4096,8192),hypothesis='Repeat availability uses entry-position maxRep1024, unlike ordinary block-end bound.'))
    for floor in (0,16,32):
        add(Recipe(f'prefix-floor-{floor}','low-bound',b'abcd'*40,prefix=64,prefix_floor=floor,repeats=(32,64,128),seeds=(0,16,32)))
    for length in (65534,65535,65536,65537,65538,65539,65540,65541,65542,65700,131072):
        add(Recipe(f'long-match-{length}','long-length',b'A'*length,window_log=17))
    add(Recipe('long-literal-candidate','long-length',unique+unique[:2048],window_log=17,hash_log=18,hypothesis='Long literal-run candidate; only mark LL extension once actual store metadata confirms it.'))
    expected_families={'width-dispatch':30,'conventional-table':12,'short-tail':28,
                       'acceleration':25,'collision':5,'conditional-store':2,'extension':8,
                       'repeat-transition':12,'low-bound':8,'long-length':12}
    actual_families={name:sum(r.family==name for r in result) for name in {r.family for r in result}}
    require(exact(actual_families,expected_families),'frozen complete directed family counts')
    require(len(result)==142<=MAX_CASES and len({r.name for r in result})==142,'frozen unique recipe count')
    for item in result:item.packet()
    return result


def replay(case, row):
    require(type(row) is dict,'case object')
    expected_scalar={'record':'case','id':case.name,'prefix_bytes':case.prefix,'block_bytes':case.block,
                     'source_fnv1a64':fnv(case.source),'window_log':case.window_log,'hash_log':case.hash_log,
                     'min_match':case.min_match,'key_bytes':case.width,'target_length':case.target,
                     'prefix_floor':case.prefix_floor,'initial_dict_limit':case.prefix_floor+2,'initial_low_limit':case.prefix_floor+2,
                     'seeds':list(case.seeds),'table_entries':1<<case.hash_log,'physical_leading_bytes':64,'physical_suffix_bytes':262656,
                     'sequence_capacity':case.block//3+1,'literal_slack_bytes':32,'initial_repeats':list(case.repeats),
                     'replay_equal':True,'guards_intact':True}
    dynamic={'padding','parser_repeats','wire_repeats','long_length_type','long_length_position','trailing_literals','stored_literals_hex','sequences','table_before_nonzero','table_changes','table_before_le32_fnv1a64','table_after_le32_fnv1a64'}
    require(set(row)==set(expected_scalar)|dynamic,'exact case schema')
    require(exact({k:row[k] for k in expected_scalar},expected_scalar),'complete scalar/recipe metadata')
    require(type(row['padding']) is int and row['padding'] in (106,195),'padding domain')
    for name in ('parser_repeats','wire_repeats'):
        require(type(row[name]) is list and len(row[name])==3 and all(type(x) is int and 0<x<1<<32 for x in row[name]),'repeat schema')
    for name in ('long_length_type','long_length_position','trailing_literals'):
        require(type(row[name]) is int and 0<=row[name]<=MAX_BLOCK,'length metadata schema')
    require(type(row['stored_literals_hex']) is str and len(row['stored_literals_hex'])<=case.block*2,'literal byte cap')
    literals=bytes.fromhex(row['stored_literals_hex']);result=bytearray(case.source[:case.prefix]);cursor=0;reps=list(case.repeats)
    sequences=row['sequences'];require(type(sequences) is list and len(sequences)<=case.block//3+1,'sequence count cap')
    observed={'explicit_offsets':0,'zero_ll_repeats':0,'postmatch_keys_present':0}
    for item in sequences:
        require(type(item) is dict and set(item)=={'ll','ml','distance','off_base','intent'},'sequence schema')
        require(all(type(item[k]) is int and 0<=item[k]<1<<32 for k in ('ll','ml','distance','off_base')),'sequence integer schema')
        ll,ml,off=item['ll'],item['ml'],item['off_base']
        require(cursor+ll<=len(literals) and ml>=3 and off>0,'sequence lengths/source')
        result.extend(literals[cursor:cursor+ll]);cursor+=ll
        a,b,c=reps
        if off>3:distance=off-3;reps=[distance,a,b];observed['explicit_offsets']+=1
        elif ll and off==1:distance=a
        elif (ll and off==2) or (not ll and off==1):distance=b;reps=[b,a,c]
        else:distance=a-1 if not ll and off==3 else c;reps=[distance,a,b]
        require(distance==item['distance'] and 0<distance<=len(result),'independent wire distance')
        require(distance<=1<<case.window_log,'match exceeds selected window')
        require(len(result)-distance>=case.prefix_floor,'match precedes retained prefix floor')
        require(item['intent']==('repeat' if off<=3 else 'offset'),'explicit offset label')
        if ll==0 and off<=3:observed['zero_ll_repeats']+=1
        require(len(result)+ml<=len(case.source),'match output cap')
        for _ in range(ml):result.append(result[-distance])
    require(cursor==len(literals),'complete stored literal consumption')
    require(row['trailing_literals']==len(case.source)-len(result),'trailing literal exact coverage')
    result.extend(case.source[len(result):]);require(bytes(result)==case.source,'independent complete byte replay')
    require(row['wire_repeats']==reps and row['parser_repeats'][:2]==reps[:2] and row['parser_repeats'][2]==case.repeats[2],'separate parser/full wire repeat state')
    long=[(kind,index) for index,s in enumerate(sequences) for kind,value in ((1,s['ll']),(2,s['ml']-3)) if value>=65536]
    require(len(long)<=1 and (row['long_length_type'],row['long_length_position'])==(long[0] if long else (0,0)),'expanded long length metadata')
    entries=1<<case.hash_log;before=[0]*entries
    for position in case.seeds:before[bucket(case.source[position:position+case.width],case.hash_log)]=position+2
    require(exact(row['table_before_nonzero'],[[i,p] for i,p in enumerate(before) if p]),'ordered seed reconstruction')
    after=before.copy();changes=row['table_changes'];require(type(changes) is list and len(changes)<=entries,'table change cap')
    last=-1
    for change in changes:
        require(type(change) is list and len(change)==3 and all(type(x) is int and 0<=x<1<<32 for x in change),'table delta schema')
        index,old,new=change
        require(last<index<entries and before[index]==old and new!=old,'sorted unique exact old table delta')
        after[index]=new;last=index
    for label,table in [('before',before),('after',after)]:
        require(row[f'table_{label}_le32_fnv1a64']==fnv(b''.join(struct.pack('<I',x) for x in table)),'canonical complete table digest')
        for index,position in enumerate(table):
            if position:
                require(2<=position and position-2+case.width<=len(case.source),'complete selected table key')
                require(bucket(case.source[position-2:position-2+case.width],case.hash_log)==index,'independent key/bucket')
    end=case.prefix
    for item in sequences:
        end+=item['ll']+item['ml']
        if end<=len(case.source)-8:
            position=end-2
            if after[bucket(case.source[position:position+case.width],case.hash_log)]==position+2:
                observed['postmatch_keys_present']+=1
    observed.update({'sequences':len(sequences),'long_length_type':row['long_length_type'],'changed_slots':len(changes)})
    return observed


def validate(case,path):
    require(path.is_file() and 0<path.stat().st_size<=MAX_RECORDS,'bounded observation file')
    records=[decode_json(x) for x in path.read_text().splitlines()]
    require(len(records)==3,'one metadata/two case rows')
    metadata=records[0]
    fixed={'record':'metadata','schema_version':2,'upstream_revision':REVISION,'entry_point':'ZSTD_compressBlock_fast','observations':'sequences-and-final-table-state-not-chronological-trace','strategy':'fast','chain_log':6,'search_log':1,'loaded_dict_end':0,'dict_match_state':False,'recipe_count':1,'case_count':2,'sizeof_seqdef':8}
    require(type(metadata) is dict and set(metadata)==set(fixed)|{'pointer_bits','little_endian','sizeof_match_state'},'metadata fields')
    require(exact({k:metadata[k] for k in fixed},fixed),'metadata constants')
    require(type(metadata['pointer_bits']) is int and metadata['pointer_bits'] in (32,64) and type(metadata['little_endian']) is bool and type(metadata['sizeof_match_state']) is int and 0<metadata['sizeof_match_state']<=1<<20,'host metadata domains')
    observed=[]
    for index,row in enumerate(records[1:]):
        require(row['padding']==(106,195)[index],'padding order')
        observed.append(replay(case,row))
    require(exact({k:v for k,v in records[1].items() if k!='padding'}, {k:v for k,v in records[2].items() if k!='padding'}),'complete paired-padding independence')
    return {'id':case.name,'observations':2,'records_sha256':sha(path),'source_sha256':hashlib.sha256(case.source).hexdigest(),'observed':observed[0],'metadata':metadata}


def compare_families(cases, output, mode, rows):
    by_id={case.name:case for case in cases}
    observed={record['id']:record['observed'] for record in rows}
    comparisons=[]
    def comparable(name):
        records=[decode_json(x) for x in (output/(mode+'-'+name+'.stdout')).read_text().splitlines()]
        return records[1]
    def same(left,right,ignored):
        a=comparable(left);b=comparable(right)
        require(exact({k:v for k,v in a.items() if k not in ignored},
                      {k:v for k,v in b.items() if k not in ignored}),f'family equivalence: {left}/{right}')
        comparisons.append({'left':left,'right':right,'ignored_scalar_fields':sorted(ignored),'entire_remaining_record_equal':True})
    for case in cases:
        if case.family=='width-dispatch':
            if case.min_match==3:same(case.name,case.name.replace('-m3-','-m4-'),{'id','min_match'})
            if case.window_log==18:same(case.name,case.name.replace('-w18','-w19'),{'id','window_log'})
    same('accel-no-match-t0','accel-no-match-t1',{'id','target_length'})
    short4=comparable('conditional-store-step4');short5=comparable('conditional-store-step5')
    require(short4['sequences']==short5['sequences']==[{'ll':1,'ml':4,'distance':65,'off_base':68,'intent':'offset'}],'conditional-store second-probe match')
    case=by_id['conditional-store-step4'];position=case.prefix+4;slot=bucket(case.source[position:position+case.width],case.hash_log)
    def final_slot(record,index):
        value=dict(record['table_before_nonzero']).get(index,0)
        for changed,old,new in record['table_changes']:
            if changed==index:value=new
        return value
    require(final_slot(short4,slot)==position+2 and final_slot(short5,slot)==0,'isolated step4/5 next-key final-slot consequence')
    comparisons.append({'left':'conditional-store-step4','right':'conditional-store-step5','same_sequence':True,'isolated_next_key_bucket':slot,'step4_value':position+2,'step5_value':0,'qualification':'Final state from bounded cleanup, not chronological tracing.'})
    # These are consequences visible in the unmodified C output, not a trace
    # or a claim that every intended recipe reached a desired source branch.
    witnesses={
      'multiple_zero_literal_repeats':[name for name,value in observed.items() if value['zero_ll_repeats']>=2],
      'zero_literal_repeat':[name for name,value in observed.items() if value['zero_ll_repeats']>0],
      'postmatch_end_minus_two_key_present':[name for name,value in observed.items() if value['postmatch_keys_present']>0],
      'actual_long_literal_marker':[name for name,value in observed.items() if value['long_length_type']==1],
      'actual_long_match_marker':[name for name,value in observed.items() if value['long_length_type']==2],
      'disabled_no_new_offset':[name for name,value in observed.items() if name.endswith('-disabled') and value['explicit_offsets']==0],
      'disabled_one_new_offset':[name for name,value in observed.items() if name.endswith('-disabled') and value['explicit_offsets']==1],
      'disabled_multiple_new_offsets':[name for name,value in observed.items() if name.endswith('-disabled') and value['explicit_offsets']>=2],
    }
    return {'equivalence_pairs':comparisons,'observed_consequences':witnesses,
            'unobserved_consequences':[name for name,ids in witnesses.items() if not ids],
            'qualification':'Capture-integrity success is separate from completion of intended branch-family witnesses. Collision/step/extension causality still needs source-backed review of concrete rows.'}


def run(args):
    upstream=args.upstream.resolve();output=args.output.resolve()
    require(not output.exists() and not output.is_relative_to(HERE),'fresh run directory outside draft source')
    require(not HERE.is_relative_to(output),'output cannot contain draft source')
    output.mkdir(parents=True)
    env={k:v for k,v in os.environ.items() if k in {'PATH','HOME','TMPDIR','TMP','TEMP','SYSTEMROOT','SDKROOT','MACOSX_DEPLOYMENT_TARGET','DEVELOPER_DIR'}}|{'LC_ALL':'C','LANG':'C','TZ':'UTC'}
    cc=shutil.which(args.cc,path=env.get('PATH'));require(cc is not None,'compiler unavailable');cc=str(Path(cc).resolve())
    manifest=decode_json(args.source_manifest.read_text())
    require(manifest['upstream_revision']==REVISION,'pinned source manifest revision')
    for name,digest in manifest['source_sha256'].items():require(sha(upstream/name)==digest,f'pinned reference source {name}')
    cases=recipes();packet_dir=output/'packets';packet_dir.mkdir()
    # All source/envelope identities are frozen independently BEFORE compiling/executing C.
    for case in cases:(packet_dir/(case.name+'.packet')).write_bytes(case.packet())
    recipe_manifest={'schema_version':2,'source_generator_sha256':sha(Path(__file__)),'cases':[case.metadata() for case in cases],'case_count':len(cases),'observation_count':len(cases)*2,'status':'pre-execution deterministic recipe hypotheses'}
    write_json(output/'recipes.json',recipe_manifest)
    sources=[Path(__file__).resolve(),HERE/'fast_direct_families.c',args.source_manifest.resolve(),upstream/'lib/libzstd.a',*[upstream/name for name in manifest['source_sha256']],output/'recipes.json',*sorted(packet_dir.iterdir())]
    def state():return [{'path':str(p),'bytes':p.stat().st_size,'sha256':sha(p)} for p in sources]
    before=state();evidence={'schema_version':2,'started_at':utc(),'commands':[],'source_before':before,'status':'running','case_count':len(cases),'observation_count_per_mode':len(cases)*2,'environment':env,'compiler_executable':cc,'compiler_sha256':sha(Path(cc))}
    def save():write_json(output/'evidence.json',evidence)
    def command(label,argv,overrides=None):
        require(state()==before,'source/envelope changed before command')
        stdout=output/(label+'.stdout');stderr=output/(label+'.stderr');current_env=env|(overrides or {})
        start=utc()
        with stdout.open('w') as out,stderr.open('w') as err:p=subprocess.run(argv,cwd=output,env=current_env,stdout=out,stderr=err,check=False,timeout=120)
        evidence['commands'].append({'label':label,'command':argv,'environment':current_env,'cwd':str(output),'started_at':start,'finished_at':utc(),'exit_code':p.returncode,'stdout':stdout.name,'stderr':stderr.name,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)})
        save();require(p.returncode==0,f'command failed: {label}');require(state()==before,'source/envelope changed after command');return stdout
    save()
    try:
        require(command('reference-revision',['git','-C',str(upstream),'rev-parse','HEAD']).read_text().strip()==REVISION,'pinned revision')
        require(command('reference-status',['git','-C',str(upstream),'status','--porcelain']).read_text()=='','reference clean status')
        command('compiler-version',[cc,'--version'])
        outputs={}
        for mode in (('normal','sanitized') if args.sanitize else ('normal',)):
            executable=output/('fast-families-'+mode)
            cmd=[cc,'-std=c11','-Wall','-Wextra','-Werror','-I',str(upstream/'lib'),'-I',str(upstream/'lib/common'),'-I',str(upstream/'lib/compress')]
            if mode=='sanitized':cmd+=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',str(HERE/'fast_direct_families.c'),str(upstream/'lib/compress/zstd_fast.c')]
            else:cmd+=['-O2',str(HERE/'fast_direct_families.c')]
            cmd +=[str(upstream/'lib/libzstd.a'),'-o',str(executable)]
            command(mode+'-compile',cmd);rows=[]
            for case in cases:
                overrides={'ASAN_OPTIONS':'halt_on_error=1:detect_leaks=0','UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'} if mode=='sanitized' else {}
                path=command(mode+'-'+case.name,[str(executable),str(packet_dir/(case.name+'.packet'))],overrides)
                rows.append(validate(case,path))
                if mode=='sanitized':require(path.read_bytes()==(output/('normal-'+case.name+'.stdout')).read_bytes(),'normal/sanitized entire observation equality')
            outputs[mode]={'validated_cases':len(rows),'validated_observations':sum(r['observations'] for r in rows),'executable_sha256':sha(executable),'cases':rows,'family_review':compare_families(cases,output,mode,rows)}
            evidence['results']=outputs;save()
        evidence['source_after']=state();require(evidence['source_after']==before,'final immutable source/envelopes')
        evidence['status']='passed';evidence['finished_at']=utc();evidence['instrumentation']={'helper_and_unchanged_fast_and_inline_headers':args.sanitize,'remaining_archive_instrumented':False,'leak_sanitizer':False}
        evidence['scope']='Unchanged direct parser observations, not chronological tracing, collector or ordinary-frame publication. Branch hypotheses require separate measured witness review.'
        save();print(f'Validated{len(cases)}recipes/{len(cases)*2}observations per mode: {output}')
    except BaseException:
        evidence['status']='failed';evidence['failure']=traceback.format_exc();save();raise


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream',required=True,type=Path)
    parser.add_argument('--output',required=True,type=Path)
    parser.add_argument('--source-manifest',required=True,type=Path)
    parser.add_argument('--cc',default='cc');parser.add_argument('--sanitize',action='store_true')
    args=parser.parse_args();run(args)


if __name__=='__main__':main()
