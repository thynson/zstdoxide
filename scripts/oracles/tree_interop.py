#!/usr/bin/env python3
"""Source-only binary-tree public inputs with independently resolved new tuples."""
from pathlib import Path
from dataclasses import replace
import collections
import csv
import hashlib
import io
ROOT=Path(__file__).resolve().parents[2]
REVISION='f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_SOURCE=4<<20
MAX_FRAME=5<<20
MAX_TOTAL_SOURCE=256<<20
PINS={'scripts/oracles/lazy_interop.py': 'db2f4e9568eff5a2508e7b65e5106404fc03c957ea8cf0afb7d3ab887b56d937', 'scripts/oracles/tree_frames.py': '7a4bdbc669c8d0a512870fd6198ae94e780af3f7540e265cea42c84cd1d28353', 'scripts/check_reference_admission_v5.py': 'f016f23bf50a00a529276e506684c4cee1129aebbe222ca5b812cff34537287b', 'scripts/check_lazy_interop_capture_v2.py': '0cb1ea0cef617b732c02537dfda09f5d4ac3080d2f3e2dccab6cb2c399d8410f', 'docs/reference-tree-interop-protocol.md': 'f85efec2c272e6bf02fd3853dd402ef192c9b01f097a38f5f9392f7bc79f8037', 'tests/fixtures/reference-admission-v5/README.md': '9a0f073716215e978c3d18eb0be1c3be665ab5bb4cfeed931396aceff9854a2d', 'tests/fixtures/reference-admission-v5/broader.tsv': '987997de3241abd0c07f149f120eb432c2d70ba53a65582a0ea0ae865a3602eb', 'tests/fixtures/reference-admission-v5/fuzz.tsv': '3f6cc5d182eef4da8ac8919bbe9c26e06a62148acdeb41ce03a12381598c717d', 'tests/fixtures/reference-admission-v5/greedy.tsv': '48aa1e7d6155b3cb9a36ca441dafa9a63e894f97e4b1d6b463387772dc1ace24', 'tests/fixtures/reference-admission-v5/interop.tsv': '8d13b0ba3d264e9ea76c3d437dbfc6a7c568c7cd356d7b8b2d7d50edb9a625be', 'tests/fixtures/reference-admission-v5/lazy.tsv': '2c210de372c6f918c55bd7fb8883290a8f0cdfc5d2d7b91f81b4182dd21edd20', 'tests/fixtures/reference-admission-v5/loops.tsv': 'a7725e10501a9190d53c0d15b2ff746f873502f293195f4e335474b4694f3be2', 'tests/fixtures/reference-admission-v5/provenance.json': 'be264e270bac401b405bd6b18a58cefbc624b59d88ad54e1948abbd55b10c6bc'}

def require(value,reason):
    if not value:raise ValueError(reason)
def sha(b):return hashlib.sha256(b).hexdigest()
def checked_inputs():
    for n,h in PINS.items():
        p=ROOT/n;require(p.is_file() and not p.is_symlink() and sha(p.read_bytes())==h,'pinned tree public input '+n)
def rows(n):return list(csv.DictReader(io.StringIO((ROOT/n).read_text()),delimiter='\t'))
def recipes():
    checked_inputs()
    from oracles import lazy_interop as prior, tree_frames as ordinary, double_fast_interop as common
    import check_reference_admission_v5 as policy
    from check_lazy_interop_capture_v2 import presets,resolved_new_request
    table=presets();result=[];old=prior.recipes();mapping=rows('tests/fixtures/reference-admission-v5/lazy.tsv')
    require(len(old)==len(mapping)==886,'complete predecessor public set')
    for c,r in zip(old,mapping,strict=True):
        require(c.name==r['case'] and sha(c.source)==r['source_sha256'] and c.request==tuple(map(int,r['request'].split(','))) and c.expected==r['v4'],'unchanged predecessor bytes')
        result.append(replace(c,expected=r['v5']))
    def add(name,source,request,why):
        c=common.Recipe(name,source,request,'unsupported',why)
        resolved=resolved_new_request(c,table)
        result.append(replace(c,expected=policy.classify(resolved)))
    for c in ordinary.recipes():
        add('tree-'+c.name,c.source,common.request(level=9,window=c.window_log,chain=c.chain_log,table=c.hash_log,search=c.search_log,minimum=c.min_match,target=c.target,strategy=6),'Frozen ordinary tree source; independent public known-size adjustment, no internal schedule claim.')
    source=bytes(range(251))*1045
    for size in (0,16384,16385,131072,131073,262144,262145):
        for level in range(9,16):
            add(f'tree-level-{level}-size-{size}',source[:size],common.request(level=level),'Unchanged default level at exact known-size preset boundaries.')
    boundary=source[:262145]
    for label,kw in [('baseline',{}),('window21',dict(window=21)),('window22',dict(window=22)),('hash19',dict(table=19)),('hash20',dict(table=20)),('chain19',dict(chain=19)),('chain20',dict(chain=20)),('search8',dict(search=8)),('search9',dict(search=9)),('target0',dict(target=0)),('target4',dict(target=4)),('targetmax',dict(target=131072)),('width3',dict(minimum=3)),('width4',dict(minimum=4)),('hint1',dict(hint=1)),('hintmax',dict(hint=(1<<31)-1))]:
        values=dict(level=9,window=19,chain=19,table=19,search=4,minimum=4,target=0,strategy=6);values.update(kw)
        add('tree-boundary-'+label,boundary,common.request(**values),'Explicit public tree bound or complete-frame equivalence hypothesis.')
    large=(bytes(range(251))*8356)[:(1<<21)+1]
    add('tree-boundary-window22-resource',large,common.request(level=9,window=22,chain=19,table=19,search=4,minimum=4,strategy=6),'Actual source exceeds the W21 cap after known-size adjustment.')
    for c in result:c.metadata()
    require(len(result)==len({c.name for c in result})==1197,'unique successor inventory')
    require(sum(len(c.source) for c in result)<=MAX_TOTAL_SOURCE,'aggregate input bound')
    return result

def manifest():
    from oracles import lazy_interop as prior
    from check_lazy_interop_capture_v2 import presets,resolved_new_request
    cases=recipes();table=presets();old=rows('tests/fixtures/reference-admission-v5/lazy.tsv')
    equal=prior.manifest()['expected_rust_frame_equivalence']+[['tree-boundary-'+n for n in ('baseline','target0','target4','targetmax','width3','width4','hint1','hintmax')]]
    by_name={c.name:c for c in cases}
    for group in equal:
        require(all(by_name[n].expected=='success' for n in group),'admitted equivalence')
        require(len({sha(by_name[n].source) for n in group})==1,'equal source bytes')
    tuples=[list(map(int,r['resolved_tuple'].split(','))) for r in old]+[resolved_new_request(c,table) for c in cases[886:]]
    return dict(schema_version=5,upstream_revision=REVISION,status='Source-only pre-execution inputs, tuples and equivalence hypotheses.',source_limit=MAX_SOURCE,frame_limit=MAX_FRAME,aggregate_source_limit=MAX_TOTAL_SOURCE,source_bytes=sum(len(c.source) for c in cases),recipe_count=len(cases),classifications=dict(collections.Counter(c.expected for c in cases)),input_sha256=PINS,request_fields=['level','window','chain','hash','search','minimum','target','strategy','hint'],recipes=[c.metadata() for c in cases],resolved_tuples=tuples,expected_rust_frame_equivalence=equal,families=dict(historical=886,ordinary_tree=245,numeric_boundaries=49,explicit_boundaries=17),qualification='No C parameter observation or identical C/Rust entropy claim. Newly admitted historical and all 311 new request tuples independently reconstructed before codec execution.')
