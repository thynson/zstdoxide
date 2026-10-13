#!/usr/bin/env python3
"""Capture reviewed ordinary tree frames with bounded immutable evidence."""

from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import sys
import traceback

ROOT = Path(__file__).resolve().parents[1]
PINS = {'scripts/check_tree_frame_recipes.py': '5eb2cfed52c665213c4536c13d0955ae74ad78ce503166b7e9a7a65e40530890', 'scripts/oracles/tree_frames.py': '7a4bdbc669c8d0a512870fd6198ae94e780af3f7540e265cea42c84cd1d28353', 'tests/fixtures/reference-tree-frame-recipes/recipes.json': 'ce758ddf23bb87a8e4862e1c1b5ad251977d2c3d14436c51ad21cd7da019548e', 'scripts/oracles/tree_frame_v1.c': '8625b969eb3ba3871271ddb56ab835ec1d9581b3a2c4461b682f06769db8e85e', 'scripts/oracles/tree_frame_validation.py': '91bb1f8e4c0ea00ec096788287892dd86da5a03758465eed9ee0a6b16806e51f', 'scripts/check_tree_frame_validator.py': '9b05bf897c8965d98fe7d2e27615e502ad16d95092497cb093de60569299c940', 'scripts/capture_double_fast_tiny.py': '5dea723222db1171e6302411d4d59870e01a09dfd7df74bf47c51cc6978bef19', 'scripts/oracles/double_fast_direct.py': '5280b94e6e10e34a263b7ae1b31a24026adfd92327767b776e81344362604065', 'scripts/oracles/fast_frame_publication_support.py': 'c1a205a9b7f897232dd7eb1e177f2a318c42da77ac8348226825732b44b0857a', 'docs/reference-tree-frame-helper-review.md': '101a15b22764c00af220e70df2f2a57b4a86f230335539ccf2d44c1bd36767c5', 'docs/reference-tree-frame-oracle.md': '4357ec5e4cf813a387bed53acb89a01e9fb048f1fe0607827d2a71b2b8715a34', 'docs/evidence/2026-10-13-tree-frame-helper.json': '3805044e64539ed91c09334628e5afe7e8c35d16ff5aacbb81e4edb67296f5c6', 'docs/evidence/2026-10-13-tree-frame-recipes.json': '8809068e76991cae9e3e70af8cfc118bec42f27ec1e73d6deb6a085995af63b2'}

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def load_inputs():
    from types import SimpleNamespace
    for name,digest in PINS.items():
        p=ROOT/name
        require(p.is_file() and not p.is_symlink() and p.stat().st_size<=64<<20 and sha(p)==digest,'reviewed input identity: '+name)
    import check_tree_frame_recipes as recipes
    import capture_double_fast_tiny as bounded
    from oracles import tree_frames as generator, tree_frame_validation as validator
    recipes.check()
    cases=generator.recipes();require(len(cases)==245,'complete recipe count')
    descriptor=SimpleNamespace(GENERATOR=ROOT/'scripts/oracles/tree_frames.py',MANIFEST=recipes.DEST)
    return descriptor,None,bounded,validator,generator,cases


def malformed(inputs,frames):
    source=str(inputs/'bt_chain_first_uniform.source');result=[]
    def frame(name,change):
        output=frames/('negative-'+name+'.zst')
        argv=['frame','bt_negative_'+name,source,'4096',str(output),'-','14','10','10','4','4','0','6','106']
        change(argv);result.append((name,argv,output))
    for name,index,value in [
        ('window_low',6,'9'),('window_high',6,'22'),('hash_low',7,'5'),('hash_high',7,'20'),
        ('chain_low',8,'5'),('chain_high',8,'20'),('search_low',9,'0'),('search_high',9,'9'),
        ('minimum_low',10,'2'),('minimum_high',10,'8'),('target_high',11,'131073'),
        ('strategy_low',12,'5'),('strategy_high',12,'7'),('strategy_bool',12,'true'),
        ('padding_low',13,'105'),('padding_high',13,'196'),('padding_negative',13,'-1'),
        ('negative_scalar',7,'-1'),('float_scalar',7,'6.0'),('leading_zero',7,'06'),
        ('narrowing',7,'4294967296'),('overflow_scalar',7,'18446744073709551616'),
        ('case_charset',1,'bt_BAD'),('case_prefix',1,'d2_negative'),('case_length',1,'x'*64),
        ('source_missing',2,str(inputs/'absent.source')),('source_oversize',2,str(inputs/'oversize.source')),
        ('schedule_short',3,'4095'),('schedule_negative',3,'-1'),('schedule_float',3,'4096.0'),
        ('schedule_trailing',3,'4096,'),('schedule_many',3,','.join(['0']*65)),
        ('block_large',3,'131073'),('window_block',6,'10')]:
        frame(name,lambda a,index=index,value=value:a.__setitem__(index,value))
    frame('missing_padding',lambda a:a.pop());frame('extra_argument',lambda a:a.append('extra'))
    for name,pledge,window,strategy in [('header_unknown','18446744073709551615','21','6'),('header_overflow','18446744073709551616','21','6'),('header_negative','-1','21','6'),('header_window_low','1','9','6'),('header_window_high','1','22','6'),('header_strategy','1','21','5')]:
        result.append((name,['header',pledge,window,strategy],None))
    result.extend([('bad_mode',['unknown'],None),('metadata_extra',['metadata','extra'],None)])
    return result


def plan():
    *_,generator,cases=load_inputs();negatives=malformed(Path('/planned-inputs'),Path('/planned-frames'))
    require(len(negatives)==len({n for n,_,_ in negatives})==44,'negative inventory')
    return dict(recipes=len(cases),source_bytes=sum(len(c.source) for c in cases),scheduled_calls=sum(len(c.schedule) for c in cases),positive_calls=sum(sum(n>0 for n in c.schedule) for c in cases),whole_comparisons=sum(c.whole_compare for c in cases),incomplete_headers=len(generator.HEADERS),malformed_per_build=len(negatives),padding_passes=[106,195],expected_commands=4+2*(30+2*len(cases)+len(generator.HEADERS)+len(negatives)),codec_executions=0,status='pre-execution plan checked')


def run(args):
    recipes, floor_recipes, bounded, validator, generator, cases = load_inputs()
    upstream, output = args.upstream.resolve(), args.output.resolve()
    for path in (ROOT, upstream, recipes.MANIFEST, Path(__file__).resolve()):
        require(bounded.disjoint(output, path), 'output overlaps protected input')
    require(bounded.disjoint(ROOT, upstream) and upstream.is_dir(), 'external reference required')
    require(not output.exists(), 'fresh output directory required')
    cc = shutil.which(args.cc)
    require(cc is not None, 'compiler unavailable')
    cc = str(Path(cc).resolve())
    catalog = validator.parse_json((ROOT / 'tests/upstream/manifest.json').read_bytes())
    require(catalog['upstream']['revision'] == generator.REVISION, 'reference revision')
    source_map = {r['path']: r['sha256'] for r in catalog['sources'] + catalog['excluded_sources']
                  if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    for name, digest in source_map.items():
        require(sha(upstream / name) == digest, 'pinned C source/header: ' + name)
    c_sources = sorted(p for folder in ('common', 'compress', 'decompress') for p in (upstream / 'lib' / folder).glob('*.c'))
    require(len(c_sources) == 26 and all(str(p.relative_to(upstream)) in source_map for p in c_sources), 'complete used C source list')
    output.mkdir(parents=True)
    inputs = output / 'inputs'; inputs.mkdir()
    for case in cases:
        (inputs / (case.name + '.source')).write_bytes(case.source)
    (inputs / 'recipes.json').write_bytes(recipes.MANIFEST.read_bytes())
    (inputs / 'oversize.source').write_bytes(bytes((4 << 20) + 1))
    protected = [Path(__file__).resolve(), ROOT / 'tests/upstream/manifest.json', recipes.GENERATOR, recipes.MANIFEST,
                 ROOT / 'scripts/oracles/fast_frame_publication_support.py',
                 *[ROOT / name for name in PINS], *[upstream / name for name in sorted(source_map)], *sorted(inputs.iterdir())]
    def state():
        return [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in protected]
    original = state()
    environment = {'PATH': os.environ.get('PATH', os.defpath), 'LC_ALL': 'C', 'LANG': 'C', 'TZ': 'UTC'}
    if 'TMPDIR' in os.environ:
        environment['TMPDIR'] = os.environ['TMPDIR']
    evidence = {'schema_version': 1, 'status': 'running', 'started_at': bounded.utc(), 'reference_revision': generator.REVISION,
                'source_before': original, 'reference_source_sha256': source_map, 'support_archive_used': False,
                'compiler': cc, 'compiler_sha256': sha(Path(cc)), 'commands': [], 'results': {},
                'scope': 'Ordinary explicit-parameter internal-begin frame publication. No collector or public parameter-resolution claim.'}
    def save():
        bounded.write_json(output / 'evidence.json', evidence)
    def command(label, argv, overrides=None, timeout=180, expected_failure=False):
        require(state() == original, 'source changed before ' + label)
        row = bounded.run_command(argv, output, label, environment | (overrides or {}), timeout=timeout)
        evidence['commands'].append(row); save()
        require('error' not in row and row.get('exit_code') == (1 if expected_failure else 0), 'unexpected command outcome: ' + label)
        if expected_failure:
            error = (output / row['stderr']['path']).read_bytes()
            require(error.startswith(b'frame oracle [') and b'ERROR: AddressSanitizer' not in error
                    and b'runtime error:' not in error, 'clean expected helper rejection')
        require(state() == original, 'source changed after ' + label)
        return output / row['stdout']['path']
    def records(path):
        require(path.stat().st_size <= 64 << 20, 'bounded records')
        return [validator.parse_json(line) for line in path.read_bytes().splitlines()]
    def frame_bytes(path, case):
        bound = len(case.source) + 3 * (sum(n > 0 for n in case.schedule) + 1) + 22
        require(path.is_file() and not path.is_symlink() and path.stat().st_size <= bound,
                'bounded regular frame output')
        return path.read_bytes()
    save()
    try:
        path = command('reference-revision', ['git', '-C', str(upstream), 'rev-parse', 'HEAD'])
        require(path.read_text().strip() == generator.REVISION, 'actual reference revision')
        path = command('reference-status', ['git', '-C', str(upstream), 'status', '--porcelain', '--untracked-files=no'])
        require(not path.read_bytes(), 'tracked reference changed')
        command('compiler-version', [cc, '--version'])
        command('compiler-target', [cc, '-dumpmachine'])
        helper = ROOT / 'scripts/oracles/tree_frame_v1.c'
        common = [cc, '-std=c11', '-g', '-I', str(upstream / 'lib'), '-I', str(upstream / 'lib/common'),
                  '-I', str(upstream / 'lib/compress'), '-UZSTD_MULTITHREAD', '-DZSTD_DISABLE_ASM', '-DZSTD_LEGACY_SUPPORT=0', '-DXXH_NAMESPACE=ZSTD_']
        for mode in ('normal', 'sanitized'):
            build = output / ('build-' + mode); build.mkdir()
            frames = output / ('frames-' + mode); frames.mkdir()
            flags = ['-O2'] if mode == 'normal' else ['-O1', '-fsanitize=address,undefined', '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']
            dependencies = command(mode + '-dependencies', common + flags + ['-MM', str(helper)] + [str(p) for p in c_sources])
            dependency_paths = set()
            for line in dependencies.read_text().replace('\\\n', ' ').splitlines():
                for name in line.split(':', 1)[1].split():
                    path = Path(name).resolve(); dependency_paths.add(path)
                    require(path == helper or (path.is_relative_to(upstream) and str(path.relative_to(upstream)) in source_map), 'unexpected non-system dependency')
            dependency_map = {str(p): sha(p) for p in sorted(dependency_paths)}
            command(mode + '-predefines', common + flags + ['-dM', '-E', str(helper)])
            objects = []
            for index, source in enumerate(c_sources):
                obj = build / ('upstream-' + str(index) + '.o'); objects.append(obj)
                command(mode + '-compile-' + str(index), common + flags + ['-c', str(source), '-o', str(obj)])
            executable = build / 'tree-frame-v1'
            command(mode + '-link', common + flags + ['-Wall', '-Wextra', '-Werror', '-Wno-deprecated-declarations',
                    str(helper)] + [str(p) for p in objects] + ['-o', str(executable)])
            overrides = {'ASAN_OPTIONS': 'detect_leaks=0:halt_on_error=1', 'UBSAN_OPTIONS': 'halt_on_error=1:print_stacktrace=1'} if mode == 'sanitized' else {}
            metadata_path = command(mode + '-metadata', [str(executable), 'metadata'], overrides)
            metadata = records(metadata_path)
            require(len(metadata) == 1, 'one native metadata record')
            validator.validate_metadata(metadata[0])
            evidence['results'][mode] = {'executable_sha256': sha(executable), 'dependencies': dependency_map,
                'compiled_codec_sources': [str(p.relative_to(upstream)) for p in c_sources],
                'instrumented_codec_sources': [str(p.relative_to(upstream)) for p in c_sources] if mode == 'sanitized' else [],
                'helper_instrumented': mode == 'sanitized', 'leak_sanitizer': False, 'assembly': False,
                'metadata': metadata[0], 'cases': [], 'headers': [], 'malformed': []}
            save()
            for index, case in enumerate(cases):
                baseline=None
                for padding in [106,195]:
                    suffix=case.name+'-p'+str(padding)
                    frame=frames/(suffix+'.zst');whole=frames/(suffix+'.whole.zst') if case.whole_compare else None
                    schedule=','.join(str(n) for n in case.schedule) or '-'
                    argv=[str(executable),'frame',case.name,str(inputs/(case.name+'.source')),schedule,str(frame),str(whole) if whole else '-',*map(str,(case.window_log,case.hash_log,case.chain_log,case.search_log,case.min_match,case.target,6,padding))]
                    path=command(mode+'-'+suffix,argv,overrides)
                    require(not (output/(mode+'-'+suffix+'.stderr')).read_bytes(),'unexpected C/sanitizer diagnostics')
                    logical=records(path);fb=frame_bytes(frame,case);wb=frame_bytes(whole,case) if whole else None
                    summary=validator.validate_case(case,logical,fb,wb)
                    current=(path.read_bytes(),fb,wb)
                    if baseline is None:baseline=current
                    else:require(current==baseline,'complete source-padding equality')
                    if mode=='sanitized':
                        require(path.read_bytes()==(output/('normal-'+suffix+'.stdout')).read_bytes(),'complete build observation equality')
                        for item in [frame]+([whole] if whole else []):require(frame_bytes(item,case)==frame_bytes(output/'frames-normal'/item.name,case),'complete build frame equality')
                    summary.update(padding=padding,stdout_sha256=sha(path),frame_sha256=sha(frame),frame_bytes=frame.stat().st_size,whole_sha256=sha(whole) if whole else None,whole_bytes=whole.stat().st_size if whole else None)
                    evidence['results'][mode]['cases'].append(summary);save()
                if index%5==4 or index+1==len(cases):print(mode+': '+str(index+1)+'/245 ordinary tree cases',flush=True)
            for index,(pledge,window) in enumerate(generator.HEADERS):
                path=command(mode+'-header-'+str(index),[str(executable),'header',str(pledge),str(window),'6'],overrides)
                validator.validate_partial(records(path),pledge,window)
                if mode=='sanitized':require(path.read_bytes()==(output/('normal-header-'+str(index)+'.stdout')).read_bytes(),'partial header equality')
                evidence['results'][mode]['headers'].append(dict(pledge=pledge,window_log=window,strategy=6,stdout_sha256=sha(path)));save()
            for name, argv, forbidden_output in malformed(inputs, frames):
                require(forbidden_output is None or not forbidden_output.exists(), 'fresh malformed destination')
                path = command(mode + '-negative-' + name, [str(executable)] + argv, overrides, timeout=30, expected_failure=True)
                require(forbidden_output is None or not forbidden_output.exists(), 'malformed command created output')
                evidence['results'][mode]['malformed'].append({'id':name, 'stdout_sha256':sha(path)})
                save()
        require(evidence['results']['normal']['metadata'] == evidence['results']['sanitized']['metadata'], 'native metadata build equality')
        evidence['source_after'] = state(); require(evidence['source_after'] == original, 'final immutable inputs')
        evidence.update(status='passed', finished_at=bounded.utc(), normal_sanitized_equal=True)
        save(); print('Captured 245 ordinary tree recipes with two padding passes, 120 whole comparisons, 15 incomplete headers and 44 malformed rejections per build.', flush=True)
    except BaseException:
        evidence.update(status='failed', failure=traceback.format_exc(), finished_at=bounded.utc()); save(); raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check-plan', action='store_true')
    parser.add_argument('--upstream', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--cc', default='clang')
    args = parser.parse_args()
    if args.check_plan:
        print(json.dumps(plan(), sort_keys=True))
    else:
        if args.upstream is None or args.output is None:
            parser.error('--upstream and --output required')
        run(args)


if __name__ == '__main__':
    main()
