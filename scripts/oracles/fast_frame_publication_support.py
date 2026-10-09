"""Pure-Python normal Fast fixture inspection; no compiler or subprocess imports.

Recipes and scalar replay are attributed test scaffolding for pinned zstd1.5.7.
Full compressed payload decoding is recorded C capture evidence, not this module.
"""
import hashlib
import json
import zlib

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
MASK = (1 << 32) - 1
MAX_COMPRESSED_BYTES = 4 << 20
MAX_JSON_BYTES = 16 << 20
HEADERS = [(n,20) for n in (255,256,65791,65792,(1<<32)-2,(1<<32)-1,1<<32)]
HEADERS += [(1024,10),(1025,10),(1048576,20),(1048577,20)]


def need(ok, reason):
    if not ok:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + "\n").encode()


def parse_json(data):
    def pairs(items):
        out = {}
        for key, value in items:
            need(key not in out, f"duplicate JSON key {key}")
            out[key] = value
        return out
    def reject(value):
        raise ValueError(f"non-JSON number {value}")
    return json.loads(data, object_pairs_hook=pairs, parse_constant=reject)


def noise(seed, count):
    out = bytearray()
    for _ in range(count):
        seed ^= (seed << 13) & MASK
        seed ^= seed >> 17
        seed ^= (seed << 5) & MASK
        seed &= MASK
        out.append(seed & 255)
    return bytes(out)


def repeated(seed, period, size):
    unit = noise(seed, period)
    return (unit * ((size + period - 1) // period))[:size]


def recipes():
    rows = []
    def add(name, source, schedule, w=19, h=14, m=4, t=0, whole=False):
        need(sum(schedule) == len(source) <= 2 << 20, "recipe size")
        rows.append(dict(id=name, source=source, schedule=schedule, window=w,
                         hash=h, min_match=m, target=t, whole_compare=whole))
    periodic = repeated(0x13579bdf, 257, 4096)
    a, b = noise(0x2468ace1, 131072), noise(0xfedcba91, 131072)
    literals = bytes(x & 7 for x in noise(0x55667789, 4096)) + noise(0x19283745, 1024)
    literals += bytes(x & 7 for x in noise(0x89abcdef, 4096))
    add("empty", b"", [])
    add("first_uniform", bytes([0x5a])*4096, [4096], whole=True)
    add("empty_then_first_uniform", bytes([0x5a])*4096, [0, 4096])
    add("compressed_uniform_repeat", periodic + bytes([0xa6])*4096 + periodic, [4096]*3)
    add("raw_then_copy", a+a, [131072]*2, h=18, whole=True)
    add("mixed_fallback", periodic + noise(0x7654321f,4096) + bytes([0x37])*4096 + periodic, [4096]*4)
    add("tiny_stale", periodic + bytes([0x99])*6 + noise(0x10203041,7) + bytes([0xee])*5 + periodic,
        [4096,6,7,5,4096])
    add("literal_tree", literals, [4096,1024,4096], m=7)
    add("negative_literal_policy", literals, [4096,1024,4096], m=7, t=4)
    add("window_expiry_17", a+b+a, [131072]*3, w=17, h=18, whole=True)
    add("window_retained_19", a+b+a, [131072]*3, w=19, h=18, whole=True)
    for length in (4,8,12):
        for distance in (128,192,256,320):
            middle = bytearray(noise(0xabcdef01,1024))
            middle[distance:distance+length] = middle[32:32+length]
            add(f"raw_candidate_k{length}_d{distance}", periodic+middle+periodic, [4096,1024,4096], h=18)
    need(len(rows) == 23 and len({x['id'] for x in rows}) == 23, "exact unique recipe count")
    return rows


def fingerprint(data):
    h = 0xcbf29ce484222325
    for value in data:
        h = ((h ^ value) * 0x100000001b3) & ((1 << 64)-1)
    return f"{h:016x}"


def key_bucket(source, position, width, log):
    # Independent exact-width integer form of the attributed upstream hash4..7
    # definitions, already frozen in the separate match-search C collection.
    word = int.from_bytes(source[position:position+width], 'little')
    if width == 4:
        return ((word * 2654435761) & MASK) >> (32-log)
    primes = {5: 889523592379, 6: 227718039650203, 7: 58295818150454627}
    return (((word << (8*(8-width))) * primes[width]) & ((1 << 64)-1)) >> (64-log)


def frame_blocks(data):
    need(len(data) >= 6 and data[:4] == bytes.fromhex("28b52ffd"), "frame magic")
    descriptor = data[4]
    single = (descriptor >> 5) & 1
    code = descriptor >> 6
    fcs = single if code == 0 else (2 if code == 1 else 4 if code == 2 else 8)
    need(descriptor & 31 == 0, "unexpected flags")
    cursor = 5 + (not single) + fcs
    out = []
    while True:
        need(cursor + 3 <= len(data), "block header extent")
        word = int.from_bytes(data[cursor:cursor+3], "little")
        kind, field, last = (word >> 1) & 3, word >> 3, word & 1
        need(kind != 3 and field <= 131072, "block kind/size")
        cursor += 3
        size = 1 if kind == 1 else field
        need(cursor + size <= len(data), "block payload extent")
        out.append(dict(kind=kind, field=field, last=last, payload=data[cursor:cursor+size]))
        cursor += size
        if last:
            break
    need(cursor == len(data), "single frame exhaustion")
    return out


def validate_case(row, records, frame):
    source = row['source']
    begins = [r for r in records if r['type'] == 'begin']
    completes = [r for r in records if r['type'] == 'complete']
    blocks = [r for r in records if r['type'] == 'block']
    headers = [r for r in records if r['type'] == 'header']
    zero_calls = [r for r in records if r['type'] == 'zero_continue']
    wholes = [r for r in records if r['type'] == 'whole_compare']
    need(len(begins)==len(completes)==len(headers)==1, "exact begin/header/complete counts")
    need(all(r.get('case')==row['id'] for r in records), "case identity")
    need(set(r['type'] for r in records) <= {'begin','complete','block','header','zero_continue','whole_compare'}, "record types")
    need(len(blocks)==sum(n>0 for n in row['schedule']) and len(zero_calls)==row['schedule'].count(0), "call counts")
    need(len(wholes)==int(row['whole_compare']), "whole comparison count")
    expected = [row['window'],12,row['hash'],1,row['min_match'],row['target'],1]
    need(begins[0]['applied']==expected and begins[0]['source_bytes']==len(source)
         and begins[0]['source_fnv64']==fingerprint(source), "source/applied metadata")
    need(begins[0]['pending_initialized'] is False and begins[0]['initial_state']=={
        'raw_repeats':[1,4,8],'tags':{'huf':0,'ll':0,'of':0,'ml':0},'huf_table':None}, 'initialized initial state only')
    need(begins[0]['block_limit']==min(131072,max(1,min(1<<row['window'],len(source)))), 'effective block limit')
    need(completes[0]['calls']==len(row['schedule']) and completes[0]['decoded_bytes']==len(source)
         and completes[0]['blocks']==len(blocks) and completes[0]['frame_bytes']==len(frame)
         and completes[0]['frame_fnv64']==fingerprint(frame) and completes[0]['terminal_empty_raw'] is True, "completion metadata")
    wire_blocks = frame_blocks(frame)
    need(len(wire_blocks)==len(blocks)+1 and wire_blocks[-1]==dict(kind=0,field=0,last=1,payload=b''), "terminal empty block")
    expected_types=['begin']
    header_seen=False
    for size in row['schedule']:
        if not header_seen:
            expected_types.append('header'); header_seen=True
        expected_types.append('block' if size else 'zero_continue')
    if not header_seen: expected_types.append('header')
    expected_types.append('complete')
    if row['whole_compare']: expected_types.append('whole_compare')
    need([r['type'] for r in records]==expected_types, 'chronological record order')
    need([r['call'] for r in blocks]==[i for i,n in enumerate(row['schedule']) if n], 'positive call ordinal')
    need([r['call'] for r in zero_calls]==[i for i,n in enumerate(row['schedule']) if not n], 'zero call ordinal')
    semantic=[1,4,8]
    previous_state=begins[0]['initial_state']
    previous_owner=0
    initialized_owners=[True,False]
    replay=bytearray()
    previous_table=[0]*(1 << row['hash'])
    cursor=0
    generation=0
    rollback=[]
    for r,actual in zip(blocks,wire_blocks):
        length=row['schedule'][r['call']]
        need(r['source_start']==cursor and r['source_bytes']==length and r['kind']==actual['kind'] and not actual['last'], "source/wire block mapping")
        need(r['payload_bytes']==len(actual['payload']) and r['payload_fnv64']==fingerprint(actual['payload']), "payload fingerprint")
        need(r['semantic_before']==semantic and r['before']['raw_repeats'][:2]==semantic[:2], "incoming semantic state")
        need(r['before']==previous_state and r['prev_owner_before']==previous_owner, 'context state continuity')
        need(r['first']==int(cursor==0), 'actual first-block ordering')
        need(r['pending_was_initialized'] is initialized_owners[1-previous_owner], 'pending owner validity')
        trial=semantic.copy()
        candidate=r['candidate']
        if length>=7:
            generation+=1
            initialized_owners[1-previous_owner]=True
            need(candidate['fresh'] is True, "fresh generation")
            literals=bytes.fromhex(candidate['literal_bytes'])
            need(candidate['literal_count']==len(literals) and candidate['sequence_count']==len(candidate['sequences']), "candidate counts")
            lit_cursor=0
            for ll,ml,off,d in candidate['sequences']:
                need(ll>=0 and ml>=3 and off>0 and ll<=len(literals)-lit_cursor, "sequence fields")
                replay.extend(literals[lit_cursor:lit_cursor+ll]); lit_cursor+=ll
                if off>3:
                    distance=off-3
                    trial=[distance,trial[0],trial[1]]
                else:
                    selector=off-1+(ll==0)
                    if selector==0:
                        distance=trial[0]
                    elif selector==1:
                        distance=trial[1]; trial=[distance,trial[0],trial[2]]
                    else:
                        distance=trial[2] if selector==2 else trial[0]-1
                        trial=[distance,trial[0],trial[1]]
                need(r['window']['dict_limit']>=2, "prefix floor index")
                need(distance==d and 0<d<=len(replay) and d<=1<<row['window']
                     and len(replay)-d>=r['window']['dict_limit']-2
                     and len(replay)+ml<=cursor+length, "sequence distance/extent/window floor")
                for _ in range(ml):
                    replay.append(replay[-d])
            need(len(literals)-lit_cursor==candidate['trailing'], "trailing literal count")
            replay.extend(literals[lit_cursor:])
            need(candidate['semantic_repeats']==trial and r['candidate_state']['raw_repeats'][:2]==trial[:2], "candidate semantic replay")
        else:
            need(candidate=={'fresh':False,'previous_generation':generation} and r['candidate_state'] is None and r['kind']==0, "stale tiny candidate")
            replay.extend(source[cursor:cursor+length])
        need(len(replay)==cursor+length and replay[cursor:]==source[cursor:cursor+length], "exact incremental source reconstruction")
        swapped=r['prev_owner_before']!=r['prev_owner_after']
        need(swapped==(r['kind']==2), "normal publication owner")
        need(r['changed_candidate_first_two'] is (length>=7 and trial[:2]!=semantic[:2]), 'candidate repeat difference flag')
        if r['kind']==2:
            semantic=trial
        else:
            need(r['after']['raw_repeats']==r['before']['raw_repeats'], "raw/RLE raw repeats unchanged")
            if r['changed_candidate_first_two']:
                rollback.append(dict(call=r['call'],kind=r['kind']))
        need(r['semantic_after']==semantic and r['after']['raw_repeats'][:2]==semantic[:2], "published semantic triple")
        h=r['hash']
        need(h['entries']==len(previous_table), "hash dimension")
        before_bytes=b''.join(v.to_bytes(4,'little') for v in previous_table)
        need(h['before_fnv64']==fingerprint(before_bytes), "hash before reconstruction")
        indices=[]
        for index,old,new in h['changed']:
            need(0<=index<len(previous_table) and previous_table[index]==old and old!=new, "hash change tuple")
            indices.append(index); previous_table[index]=new
        need(indices==sorted(set(indices)), "unique ordered table changes")
        need(h['after_fnv64']==fingerprint(b''.join(v.to_bytes(4,'little') for v in previous_table)), "hash after reconstruction")
        width=max(4,row['min_match'])
        for index,value in enumerate(previous_table):
            if value:
                need(value>=2 and value-2+width<=cursor+length, "stored logical key extent")
                need(key_bucket(source,value-2,width,row['hash'])==index, "independent stored key bucket")
        previous_state=r['after']; previous_owner=r['prev_owner_after']
        cursor+=length
    need(cursor==len(source) and replay==source and completes[0]['candidate_generations']==generation, "complete independent replay")
    return dict(id=row['id'],blocks=[r['kind'] for r in blocks],rollback_witnesses=rollback,
                decoded_bytes=len(source),candidate_generations=generation,whole_compare=row['whole_compare'])

# Scalar types are checked before relational comparisons: bool is not an integer
# in this schema, even though Python makes it an int subclass.
BOOL_FIELDS = {'whole_compare','pending_initialized','fresh','changed_candidate_first_two',
               'terminal_empty_raw','completed_frame','decoded','pending_was_initialized'}
RECORD_KEYS = {
    'begin': {'type','case','source_bytes','source_fnv64','applied','block_limit','pending_initialized','initial_state'},
    'header': {'type','case','pledge','window_log','window','single_segment','fcs_bytes','bytes'},
    'zero_continue': {'type','case','call','produced','candidate_fresh'},
    'block': {'type','case','call','source_start','source_bytes','kind','payload_bytes','payload_fnv64','first',
              'prev_owner_before','prev_owner_after','pending_was_initialized','before','semantic_before','candidate',
              'candidate_state','after','semantic_after','changed_candidate_first_two','window','hash'},
    'complete': {'type','case','calls','blocks','candidate_generations','decoded_bytes','frame_bytes','frame_fnv64','terminal_empty_raw'},
    'whole_compare': {'type','case','blocks','decoded_bytes','frame_bytes','frame_fnv64'},
    'partial': {'type','completed_frame','decoded','pointer_bits','int_bits'},
}
BOOL_FIELDS.add('candidate_fresh')


def scalar_types(value, key=None, depth=0):
    need(depth<=40, 'JSON nesting cap')
    if isinstance(value, dict):
        for name,item in value.items():
            need(type(name) is str, 'JSON object key type')
            scalar_types(item,name,depth+1)
    elif isinstance(value,list):
        need(len(value)<=300000, 'JSON array cap')
        for item in value:
            scalar_types(item,None,depth+1)
    elif type(value) is bool:
        need(key in BOOL_FIELDS, f'boolean in non-boolean field: {key}')
    elif type(value) is int:
        need(key not in BOOL_FIELDS and 0<=value<1<<64, f'invalid integer field: {key}')
    else:
        need(value is None or type(value) is str, f'unsupported JSON scalar: {key}')
        need(key not in BOOL_FIELDS, f'non-boolean field: {key}')


def shape(value, keys, label):
    need(type(value) is dict and set(value)==set(keys), label+' exact fields')


def repeats_shape(value):
    need(type(value) is list and len(value)==3 and all(type(x) is int and 0<x<1<<32 for x in value), 'repeat triple')


def state_shape(value):
    shape(value, {'raw_repeats','tags','huf_table'}, 'state')
    repeats_shape(value['raw_repeats'])
    shape(value['tags'], {'huf','ll','of','ml'}, 'tags')
    need(all(type(x) is int and x in (0,1,2) for x in value['tags'].values()), 'tag range')
    if value['tags']['huf']==0:
        need(value['huf_table'] is None, 'unavailable HUF table')
    else:
        h=value['huf_table']
        shape(h, {'log','max_symbol','widths'}, 'HUF table')
        need(1<=h['log']<=12 and 0<=h['max_symbol']<=255 and len(h['widths'])==256, 'HUF dimensions')
        need(all(type(w) is int and 0<=w<=h['log'] for w in h['widths']), 'HUF width bounds')
        need(all(w==0 for w in h['widths'][h['max_symbol']+1:]), 'HUF unused alphabet')
        need(sum(1<<(h['log']-w) for w in h['widths'] if w)==1<<h['log'], 'HUF prefix completeness')


def header_info(data):
    need(type(data) is bytes and len(data)>=6 and data[:4]==bytes.fromhex('28b52ffd'), 'header bytes')
    d=data[4]; single=(d>>5)&1; code=d>>6; cursor=5
    need(d&31==0, 'reference frame flags')
    if single:
        window=None
    else:
        need(cursor<len(data), 'window descriptor')
        wd=data[cursor]; cursor+=1
        base=1<<(10+(wd>>3)); window=base+(base>>3)*(wd&7)
    count=single if code==0 else (2 if code==1 else 4 if code==2 else 8)
    need(cursor+count<=len(data), 'content size extent')
    content=int.from_bytes(data[cursor:cursor+count], 'little')+(256 if code==1 else 0)
    if single: window=content
    return dict(size=cursor+count,window=window,single_segment=single,fcs_bytes=count,pledge=content)


def validate_header(row, size, log):
    shape(row, RECORD_KEYS['header'], 'header record')
    data=bytes.fromhex(row['bytes']); parsed=header_info(data)
    need(parsed['size']==len(data) and row['pledge']==size and row['window_log']==log, 'header scope/source')
    for name in ('window','single_segment','fcs_bytes','pledge'):
        need(row[name]==parsed[name], 'independent header field: '+name)
    need(parsed['window']==(size if size<=1<<log else 1<<log), 'truthful frame window')
    need(parsed['single_segment']==int(size<=1<<log), 'single segment boundary')
    return data


def validate_record_shapes(records):
    need(type(records) is list and 2<=len(records)<=70, 'bounded record count')
    for r in records:
        need(type(r) is dict and r.get('type') in RECORD_KEYS, 'record discriminator')
        shape(r, RECORD_KEYS[r['type']], r['type'])
        if r['type']=='begin': state_shape(r['initial_state'])
        if r['type']=='block':
            need(r['first'] in (0,1) and r['prev_owner_before'] in (0,1) and r['prev_owner_after'] in (0,1), 'owner/first fields')
            state_shape(r['before']); state_shape(r['after'])
            repeats_shape(r['semantic_before']); repeats_shape(r['semantic_after'])
            c=r['candidate']
            if c['fresh']:
                shape(c, {'fresh','sequence_count','literal_count','sequences','trailing','literal_bytes','semantic_repeats'}, 'fresh candidate')
                state_shape(r['candidate_state']); repeats_shape(c['semantic_repeats'])
                need(all(type(s) is list and len(s)==4 for s in c['sequences']), 'sequence tuple dimensions')
            else:
                shape(c, {'fresh','previous_generation'}, 'stale candidate')
                need(r['candidate_state'] is None, 'unavailable candidate state')
            shape(r['window'], {'base_index_bias','low_limit','dict_limit','next_to_update','consumed'}, 'window')
            need(r['window']['base_index_bias']==2 and r['window']['low_limit']>=2 and r['window']['dict_limit']>=2, 'bound window indices')
            shape(r['hash'], {'entries','before_fnv64','after_fnv64','changed'}, 'hash')
            need(all(type(t) is list and len(t)==3 for t in r['hash']['changed']), 'hash delta tuple dimensions')


def validate_dataset(value):
    scalar_types(value)
    shape(value, {'schema_version','upstream_revision','cases','headers'}, 'dataset')
    need(value['schema_version']==1 and value['upstream_revision']==REVISION, 'schema/reference revision')
    expected=recipes()
    need(type(value['cases']) is list and len(value['cases'])==len(expected)==23, 'complete recipe count')
    summaries=[]
    for case,row in zip(value['cases'],expected):
        shape(case, {'recipe','records','frame_hex','whole_frame_hex'}, 'case')
        metadata={k:v for k,v in row.items() if k!='source'}
        metadata.update(source_size=len(row['source']),source_sha256=sha(row['source']))
        need(case['recipe']==metadata, 'exact ordered source recipe and hash')
        validate_record_shapes(case['records'])
        frame=bytes.fromhex(case['frame_hex'])
        need(len(frame)<=len(row['source'])+3*(len(row['schedule'])+1)+22, 'raw frame upper bound')
        headers=[r for r in case['records'] if r['type']=='header']
        need(len(headers)==1, 'one completed-frame header')
        need(frame.startswith(validate_header(headers[0],len(row['source']),row['window'])), 'stored frame header')
        summary=validate_case(row,case['records'],frame); summaries.append(summary)
        wire=frame_blocks(frame); source=row['source']; cursor=0
        records=[r for r in case['records'] if r['type']=='block']
        for r,b in zip(records,wire):
            size=r['source_bytes']
            if b['kind']==0: need(b['payload']==source[cursor:cursor+size], 'raw block exact bytes')
            if b['kind']==1: need(b['field']==size and b['payload']*size==source[cursor:cursor+size], 'RLE exact bytes')
            need(r['window']['consumed']==cursor+size, 'window consumed count')
            if r['kind']!=2:
                before,after=r['before'],r['after']
                need(before['huf_table']==after['huf_table'], 'raw/RLE committed HUF preservation')
                for tag in ('huf','ll','ml'): need(before['tags'][tag]==after['tags'][tag], 'raw/RLE committed tag preservation')
                need(after['tags']['of']==before['tags']['of'] or (before['tags']['of']==2 and after['tags']['of']==1), 'offset tag demotion')
            cursor+=size
        if row['whole_compare']:
            need(type(case['whole_frame_hex']) is str, 'whole frame bytes required')
            whole=bytes.fromhex(case['whole_frame_hex']); blocks=frame_blocks(whole)
            need(len(blocks)==len(wire)-1, 'whole actual block boundaries')
            for a,b in zip(wire,blocks):
                need({k:v for k,v in a.items() if k!='last'}=={k:v for k,v in b.items() if k!='last'}, 'whole block payload equality')
            observed=[r for r in case['records'] if r['type']=='whole_compare'][0]
            need(observed['frame_bytes']==len(whole) and observed['frame_fnv64']==fingerprint(whole)
                 and observed['decoded_bytes']==len(source) and observed['blocks']==len(blocks), 'whole capture counts')
            need(header_info(whole)==header_info(frame), 'whole frame header metadata')
        else: need(case['whole_frame_hex'] is None, 'no unsupported whole comparison')
    need(type(value['headers']) is list and len(value['headers'])==len(HEADERS)==11, 'complete partial header count')
    for records,(size,log) in zip(value['headers'],HEADERS):
        validate_record_shapes(records)
        need([r['type'] for r in records]==['header','partial'], 'partial header record order')
        validate_header(records[0],size,log)
        need(records[0]['case']=='header_only' and records[1]['completed_frame'] is False and records[1]['decoded'] is False, 'partial header qualification')
        need(records[1]['pointer_bits'] in (32,64) and records[1]['int_bits']==32, 'C host width qualification')
    need(sum(len(s['blocks']) for s in summaries)==64, 'exact main nonempty block count')
    return dict(full_frame_cases=23,whole_buffer_comparisons=4,partial_header_cases=11,
                main_source_bytes=sum(s['decoded_bytes'] for s in summaries),main_nonempty_blocks=64,
                full_compressed_payload_decoding='recorded pinned-C capture, not offline Python',cases=summaries)


def unpack(data):
    need(type(data) is bytes and 0<len(data)<=MAX_COMPRESSED_BYTES, 'compressed fixture cap')
    decoder=zlib.decompressobj(wbits=31)
    raw=decoder.decompress(data,MAX_JSON_BYTES+1)
    need(len(raw)<=MAX_JSON_BYTES and not decoder.unconsumed_tail, 'decompressed fixture cap')
    need(decoder.eof and not decoder.unused_data, 'single complete gzip member; no trailing bytes')
    return raw,parse_json(raw)


def normalize_host(value):
    # Only pointer width is host layout metadata. C int32 is a required contract.
    result=parse_json(canonical(value))
    for records in result['headers']:
        records[1]['pointer_bits']='observed-host-pointer-width'
    return result


def materialize(packed, frames):
    """Recreate original capture structure from fully covering binary extents."""
    shape(packed, {'schema_version','upstream_revision','cases','headers'}, 'packed dataset')
    need(type(frames) is bytes and len(frames)<=3<<20, 'binary frame pack cap')
    result={'schema_version':packed['schema_version'],'upstream_revision':packed['upstream_revision'],
            'cases':[],'headers':packed['headers']}
    cursor=0
    for case in packed['cases']:
        shape(case, {'recipe','records','frame','whole_frame'}, 'packed case')
        restored={'recipe':case['recipe'],'records':case['records']}
        for field in ('frame','whole_frame'):
            extent=case[field]
            if extent is None:
                need(field=='whole_frame', 'required complete frame')
                restored[field+'_hex']=None
                continue
            shape(extent, {'offset','size','sha256'}, 'binary frame extent')
            need(type(extent['offset']) is int and type(extent['size']) is int
                 and extent['offset']==cursor and 0<extent['size']<=len(frames)-cursor,
                 'ordered nonoverlapping complete frame extent')
            data=frames[cursor:cursor+extent['size']]
            need(sha(data)==extent['sha256'], 'individual frame identity')
            restored[field+'_hex']=data.hex()
            cursor+=extent['size']
        result['cases'].append(restored)
    need(cursor==len(frames), 'frame pack complete exhaustion')
    return result


def rust_index(packed):
    """Deterministic plain Rust records; no JSON/gzip dependency for Cargo."""
    lines=['// Generated and checked by import_fast_frame_publication.py.',
           '// Unchanged C v1.5.7 completed-frame observations; partial headers excluded.',
           '#[derive(Clone, Copy, Debug)]',
           'pub enum SourceRecipe {',
           '    Empty, Uniform, PeriodicUniformPeriodic, RawThenCopy, Mixed, Tiny,',
           '    LiteralTree, Window, RawCandidate { length: usize, destination: usize },',
           '}',
           '#[derive(Clone, Copy, Debug)]',
           'pub struct FrameExtent { pub offset: usize, pub size: usize }',
           '#[derive(Clone, Copy, Debug)]',
           'pub struct Case {',
           "    pub name: &'static str, pub recipe: SourceRecipe, pub source_size: usize,",
           '    pub frame: FrameExtent, pub whole_frame: Option<FrameExtent>,',
           '}',
           'pub const FRAME_BYTES: &[u8] = include_bytes!("frames.bin");',
           'pub const CASES: &[Case] = &[']
    names={'empty':'Empty','first_uniform':'Uniform','empty_then_first_uniform':'Uniform',
           'compressed_uniform_repeat':'PeriodicUniformPeriodic','raw_then_copy':'RawThenCopy',
           'mixed_fallback':'Mixed','tiny_stale':'Tiny','literal_tree':'LiteralTree',
           'negative_literal_policy':'LiteralTree','window_expiry_17':'Window','window_retained_19':'Window'}
    for case in packed['cases']:
        row=case['recipe']; name=row['id']
        if name in names: recipe='SourceRecipe::'+names[name]
        else:
            parts=name.split('_'); length=int(parts[-2][1:]); destination=int(parts[-1][1:])
            recipe=f'SourceRecipe::RawCandidate {{ length: {length}, destination: {destination} }}'
        def extent(e): return 'FrameExtent { offset: '+str(e['offset'])+', size: '+str(e['size'])+' }'
        whole='None' if case['whole_frame'] is None else 'Some('+extent(case['whole_frame'])+')'
        lines+=['    Case {',f'        name: "{name}",',f'        recipe: {recipe},',
                f"        source_size: {row['source_size']},",f"        frame: {extent(case['frame'])},",
                f'        whole_frame: {whole},','    },']
    lines+= ['];','']
    return '\n'.join(lines).encode()
