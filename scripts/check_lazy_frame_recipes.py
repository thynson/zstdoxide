#!/usr/bin/env python3
"""Verify frozen ordinary lazy-family hypotheses with no child execution."""
from pathlib import Path
from dataclasses import replace
import argparse
import hashlib
import json
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / 'scripts/oracles/lazy_frames.py'
GENERATOR_SHA = '7fd553c0c39270e4f12828c500fd33485eb5c6b3701062778816285511390894'
MANIFEST = ROOT / 'tests/fixtures/reference-lazy-frame-recipes/recipes.json'
MANIFEST_SHA = '752155495565f4efa295cc25c94635dd2061d002d3b7537493ff027176b3bd0e'
COMMON = ROOT / 'scripts/check_greedy_frame_recipes.py'
COMMON_SHA = '9f743ed73771914f1d2e44cdc811fb9a05a59dc44ce40e19f09bff5489a8a700'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def bounded(path, cap):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= cap, 'bounded regular input')
    return path.read_bytes()


def inputs():
    require(hashlib.sha256(bounded(GENERATOR, 128 << 10)).hexdigest() == GENERATOR_SHA, 'generator identity')
    require(hashlib.sha256(bounded(COMMON, 128 << 10)).hexdigest() == COMMON_SHA, 'common validator identity')
    from oracles import lazy_frames
    import check_greedy_frame_recipes as common
    return lazy_frames, common


def check(path=MANIFEST):
    generator, common = inputs()
    raw = bounded(path, 1 << 20)
    require(hashlib.sha256(raw).hexdigest() == MANIFEST_SHA, 'frozen ordinary lazy input manifest')
    observed = common.decode(raw)
    expected = generator.manifest()
    require(json.dumps(observed, sort_keys=True, allow_nan=False) == json.dumps(expected, sort_keys=True, allow_nan=False), 'exact regeneration')
    require((expected['recipe_count'], expected['source_bytes'], expected['scheduled_calls'],
             expected['positive_calls'], expected['whole_comparisons'], expected['predecessor_count'],
             expected['branches'], expected['depth_counts'], len(expected['header_only'])) ==
            (510, 22499784, 1401, 1377, 39, 138, {'chain': 231, 'row': 279}, [170]*3, 45), 'fixed envelope')
    old = {r.name:r.metadata() for r in generator.base.recipes()+generator.floors.recipes()}
    controls = [r for r in expected['recipes'] if r['predecessor'] is not None]
    require(len(controls) == len(old) == 138, 'complete predecessor inventory')
    for row in controls:
        legacy = {k:v for k,v in row.items() if k not in {'depth','predecessor'}}
        legacy['id'] = row['predecessor']
        require(legacy == old[row['predecessor']], 'unchanged predecessor input')
    return expected


def controls():
    generator, common = inputs()
    child_attempts = []
    def audit(event, args):
        if event in {'subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'}:
            child_attempts.append(event)
            raise ValueError('child execution forbidden')
    sys.addaudithook(audit)
    # Common boundary tests remain checked against their original frozen pack.
    inherited = common.controls()
    valid = generator.Recipe('d1_control', b'abcd'*16, (64,), 'Boundary controls.', depth=1)
    for depth in range(3):
        replace(valid, name=f'd{depth}_control', depth=depth).metadata()
    rejected = 0
    def reject(action):
        nonlocal rejected
        try:
            action()
        except (ValueError, OSError, UnicodeError):
            rejected += 1
        else:
            raise ValueError('malformed control accepted')
    for change in [{'depth':-1},{'depth':3},{'depth':True},{'depth':1.0},{'depth':'1'},
                   {'name':'d0_control'},{'name':'d2_control'},{'name':'control'},
                   {'predecessor':'control'},{'predecessor':True},
                   {'depth':0,'name':'d0_control','predecessor':'different'}]:
        reject(lambda change=change:replace(valid,**change).metadata())
    with tempfile.TemporaryDirectory(prefix='zstdoxide-lazy-frame-recipes-') as directory:
        root = Path(directory)
        path = root/'changed.json'
        raw = MANIFEST.read_bytes()
        for transform in [lambda v:v['recipes'][0].__setitem__('depth',1),
                          lambda v:v['recipes'][0]['parameters'].__setitem__(6,4),
                          lambda v:v['recipes'][0].__setitem__('predecessor',None),
                          lambda v:v['recipes'].append(v['recipes'][0]),
                          lambda v:v.__setitem__('recipe_count',True)]:
            value=common.decode(raw);transform(value)
            path.write_text(json.dumps(value))
            reject(lambda:check(path))
        for content in [raw+b' ',b'{"x":1,"x":2}',b'{"x":NaN}',bytes((1<<20)+1)]:
            path.write_bytes(content)
            reject(lambda:check(path))
        linked=root/'link.json';linked.symlink_to(MANIFEST)
        reject(lambda:check(linked))
    require(not child_attempts, 'zero child attempts')
    return {'positive_depth_controls':3,'new_rejections':rejected,'inherited':inherited,'child_attempts':len(child_attempts)}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',required=True,action='store_true')
    parser.add_argument('--manifest',type=Path,default=MANIFEST)
    parser.add_argument('--controls',action='store_true')
    args=parser.parse_args()
    manifest=check(args.manifest)
    print(json.dumps({k:manifest[k] for k in ['recipe_count','source_bytes','scheduled_calls','positive_calls','whole_comparisons','predecessor_count','branches','depth_counts']},sort_keys=True))
    if args.controls:print(json.dumps(controls(),sort_keys=True))


if __name__=='__main__':main()
