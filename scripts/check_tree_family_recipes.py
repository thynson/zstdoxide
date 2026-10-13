#!/usr/bin/env python3
"""Check frozen broader tree inputs, preserving tiny packets without C execution."""
from dataclasses import replace
from pathlib import Path
import hashlib
import json
import re
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
FIXTURE = 'tests/fixtures/reference-tree-family-recipes/recipes.json'
FIXTURE_SHA256 = 'b28cba0d6d827c82e579ce4131dfb6f5d8329094c47f409a41b47614656ce269'
PINS = {'scripts/oracles/tree_direct.py': '200c9491b82ae70598f7d5f0d375070ce858494f2c96304ba186329cd4f6d6f1', 'scripts/oracles/tree_validation.py': '59c16db4486f1a2f81eb995245c0c95cb8f07738edbe5792d5b54e00d7afcc9b', 'scripts/oracles/tree_families.py': '42292c72c63ed9572d44bfaf301922b8b8f29eb4f833721b7b866c3f1193b5f4'}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def inputs():
    for name, digest in PINS.items():
        path = ROOT / name
        require(path.is_file() and not path.is_symlink() and hashlib.sha256(path.read_bytes()).hexdigest() == digest,
                'reviewed input source: ' + name)
    from oracles import tree_direct as wire, tree_families as families, tree_validation as replay
    return wire, families, replay


def validate_case(case, wire, replay):
    case.request.packet()
    require(type(case.name) is str and re.fullmatch('[a-z0-9][a-z0-9-]{0,63}', case.name), 'bounded recipe identifier')
    require(type(case.hypothesis) is tuple and all(type(p) is tuple and len(p) == 2 and type(p[0]) is str
                                                and type(p[1]) is int for p in case.hypothesis), 'copied integer hypotheses')
    h = dict(case.hypothesis)
    require(len(h) == len(case.hypothesis), 'unique hypothesis keys')
    request = case.request
    if case.family == 'tiny-control':
        require(not h and case.name == 'tiny-' + case.control_of, 'tiny control identity')
        require(request.packet() == dict(wire.recipes())[case.control_of].packet(), 'exact predecessor packet')
        return
    require(case.control_of is None, 'new case has no predecessor identity')
    if case.family == 'long-periodic-frontier':
        require(not h and case.name == f'periodic-frontier-m{request.min_match}', 'periodic identity')
        require(request == wire.Request((b'treeKey' * 586)[:4096], (*range(128),2047,2048,4088), min_match=request.min_match), 'periodic source and schedule')
        return
    query = wire.integer(h['query'], 0, len(request.source)-8, 'hypothesis query')
    require(query in request.queries, 'hypothesis names an actual operation')
    wire.integer(h['construction_retry'],0,255,'bounded source construction')
    candidates = []
    if case.family == 'conditional-offset-cost':
        require(set(h) == {'query','near_distance','near_length','far_distance','far_length','weighted_length_gain',
                           'offset_bit_difference','gain_minus_cost','construction_retry'}, 'score hypothesis fields')
        near, far = h['near_distance'], h['far_distance']
        require(near in (28,29,61) and far in (255,511,1023) and query == 4096, 'score distances/query')
        require(case.name == f'cost-m{request.min_match}-near{near}-far{far}', 'score identity')
        require((request.window_log,request.hash_log,request.chain_log,request.search_log) == (13,10,12,8), 'score request dimensions')
        difference = (far+1).bit_length() - (near+3).bit_length()
        require((h['near_length'],h['far_length'],h['weighted_length_gain'],h['offset_bit_difference'],h['gain_minus_cost'])
                == (8,9,4,difference,4-difference), 'conditional score arithmetic')
        candidates = [(near,8),(far,9)]
    elif case.family == 'node-floor':
        require(set(h) == {'query','near_distance','near_length','far_distance','far_length','node_count','construction_retry'}, 'ring hypothesis fields')
        count = 1 << (request.chain_log-1)
        far = h['far_distance']
        require(request.chain_log in (6,7,8) and h['node_count'] == count and
                far in (count-2,count-1,count,2*count) and query == 2048, 'paired ring dimensions')
        require(case.name == f'ring-c{request.chain_log}-distance{far}' and
                (request.window_log,request.hash_log,request.search_log,request.min_match) == (12,10,8,4), 'ring identity/request')
        require((h['near_distance'],h['near_length'],h['far_length']) == (16,8,12), 'ring match geometry')
        candidates = [(16,8),(far,12)]
    elif case.family == 'window-floor':
        require(set(h) == {'query','distance','length','window_delta','construction_retry'}, 'window hypothesis fields')
        distance = h['distance']
        require(request.window_log in (10,11) and h['window_delta'] in (-1,0,1) and
                distance == (1 << request.window_log) + h['window_delta'] and
                query == 2*(1 << request.window_log)+16 and h['length'] == 16, 'window geometry')
        require(case.name == f'window-w{request.window_log}-distance{distance}' and
                (request.hash_log,request.chain_log,request.search_log,request.min_match) == (10,12,8,4), 'window identity/request')
        candidates = [(distance,16)]
    elif case.family == 'inclusive-matched-tail':
        require(set(h) == {'query','previous','length','construction_retry'} and
                (query,h['previous'],h['length'],len(request.source)) == (64,16,8,72), 'tail geometry')
        require(case.name == f'tail-match-m{request.min_match}' and request.queries == (0,64) and
                (request.window_log,request.hash_log,request.chain_log,request.search_log) == (10,6,6,1), 'tail identity/request')
        candidates = [(48,8)]
    else:
        raise ValueError('unknown family')
    for distance,length in candidates:
        previous = query-distance
        require(0 <= previous < query and previous+length <= query, 'nonoverlapping candidate extent')
        require(request.source[previous:previous+length] == request.source[query:query+length], 'exact candidate input bytes')
        if query+length < len(request.source):
            require(request.source[previous+length] != request.source[query+length], 'exact candidate terminator')
    nearest = query-min(distance for distance,_ in candidates)
    bucket = replay.key_bucket(request.source,query,request.hash_log,request.min_match)
    require(replay.key_bucket(request.source,nearest,request.hash_log,request.min_match)==bucket and all(
            replay.key_bucket(request.source,p,request.hash_log,request.min_match)!=bucket for p in range(nearest+1,query)),
            'last pre-query hash position is named nearest candidate')


def check():
    wire,families,replay = inputs()
    path = ROOT/FIXTURE
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 2 << 20,'bounded regular manifest')
    raw = path.read_bytes()
    require(len(raw) <= 2 << 20 and wire.sha(raw) == FIXTURE_SHA256,'exact frozen manifest bytes')
    replay.same(replay.parse(raw),families.manifest(),'regenerated source-only manifest')
    cases = families.recipes()
    totals, signs = {}, {'below':0,'equal':0,'above':0}
    for case in cases:
        validate_case(case,wire,replay)
        require(wire.decode_packet(case.request.packet())==case.request,'canonical packet')
        totals[case.family]=totals.get(case.family,0)+1
        if case.family=='conditional-offset-cost':
            gain=dict(case.hypothesis)['gain_minus_cost'];signs['below' if gain<0 else 'above' if gain>0 else 'equal']+=1
    require(totals=={'tiny-control':30,'conditional-offset-cost':45,'node-floor':12,'window-floor':6,
                     'inclusive-matched-tail':5,'long-periodic-frontier':5} and signs==dict(below=15,equal=15,above=15),'family and conditional-score inventory')
    rejected=[]
    def reject(name,case):
        try:validate_case(case,wire,replay)
        except (ValueError,TypeError,KeyError,IndexError):rejected.append(name)
        else:raise ValueError('accepted malformed family input '+name)
    case=next(c for c in cases if c.family=='conditional-offset-cost')
    h=dict(case.hypothesis);source=case.request.source;query=h['query'];previous=query-h['near_distance']
    for name,position,value in [('candidate-byte',previous+1,source[previous+1]^1),
                                 ('terminator',previous+8,source[query+8])]:
        b=bytearray(source);b[position]=value;reject(name,replace(case,request=replace(case.request,source=bytes(b))))
    b=bytearray(source);b[query-4:query]=source[query:query+4]
    reject('coherent-nearer-hash',replace(case,request=replace(case.request,source=bytes(b))))
    for field,value in [('query',True),('construction_retry',256),('weighted_length_gain',5),('offset_bit_difference',99),('gain_minus_cost',0.0)]:
        changed=h|{field:value};reject('hypothesis-'+field,replace(case,hypothesis=tuple(changed.items())))
    reject('duplicate-hypothesis',replace(case,hypothesis=case.hypothesis+(case.hypothesis[0],)))
    reject('changed-dimensions',replace(case,request=replace(case.request,search_log=1)))
    reject('escaping-name',replace(case,name='../case'))
    reject('false-predecessor',replace(case,control_of='noise-tail-8'))
    old=cases[0];b=bytes([old.request.source[0]^1])+old.request.source[1:]
    reject('changed-tiny-packet',replace(old,request=replace(old.request,source=b)))
    require(len(rejected)==13,'mutation inventory')
    return dict(status='passed',recipes=len(cases),source_bytes=sum(len(c.request.source) for c in cases),
                families=totals,conditional_hypotheses=signs,rejected=len(rejected),controls=rejected,
                fixture_sha256=wire.sha(raw),qualification='Source-only construction and exact predecessor inputs; no C outcome or branch witness.')


if __name__=='__main__':
    attempts=[]
    def no_children(event,_args):
        if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn','os.exec','os.spawn'):
            attempts.append(event);raise RuntimeError('child forbidden')
    sys.addaudithook(no_children)
    report=check();require(not attempts,'no child attempts');report.update(optimized=not __debug__,child_processes=0)
    print(json.dumps(report,sort_keys=True))
