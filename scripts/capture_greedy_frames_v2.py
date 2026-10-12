#!/usr/bin/env python3
"""Capture reviewed ordinary Greedy frames with bounded immutable evidence."""

from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import sys
import traceback

ROOT = Path(__file__).resolve().parents[1]
PINS = {
    'scripts/check_greedy_frame_recipes.py': '9f743ed73771914f1d2e44cdc811fb9a05a59dc44ce40e19f09bff5489a8a700',
    'scripts/oracles/greedy_frames.py': '360bf5139b7e2a886e9d2359c1ea258317238bd1906cfe9d123be33df7a0ce44',
    'tests/fixtures/reference-greedy-frame-recipes/recipes.json': 'f833a7015aba52bf9a43fc941f1e766ab0bec5ea8c8e285eb970de1f2d702f13',
    'scripts/check_greedy_frame_floor_recipes.py': 'f29c9960a5f99373a4da11f18a9b7c5be345d48a52396bcbaaef77fa2ba93578',
    'scripts/oracles/greedy_frame_floor_recipes.py': 'e30cb2c60762b333d6e4b510e9c9a5384c43a20e93209e89d52716f099bae2c4',
    'tests/fixtures/reference-greedy-frame-floor-recipes/recipes.json': '8414ea9f5f9eee715b22f473023a299331df5f8abe11f974202bda03af58cfbd',
    'scripts/oracles/greedy_frame_v3.c': '0e7505aa18263f1483320fb697d95ea616daa9b471a217c3445ef154d50598c1',
    'scripts/oracles/greedy_frame_validation.py': '8e400ccad307b562dd6a41ab02c5d00359d36431cd38a5c51040e08e3903c71f',
    'scripts/check_greedy_frame_validator.py': 'ac39ce8dc61cae6e2f08e2eb34a4fecbecf0d35c5648edfcbbba75aa9db8186c',
    'scripts/capture_double_fast_tiny.py': '5dea723222db1171e6302411d4d59870e01a09dfd7df74bf47c51cc6978bef19',
    'scripts/oracles/double_fast_direct.py': '5280b94e6e10e34a263b7ae1b31a24026adfd92327767b776e81344362604065',
    'scripts/oracles/fast_frame_publication_support.py': 'c1a205a9b7f897232dd7eb1e177f2a318c42da77ac8348226825732b44b0857a',
    'docs/reference-greedy-frame-helper-v3-review.md': 'c08e8a6c9f6290bc94c81511e1adbccd181bf32d2e6f099bb89001d08bfc7cfb',
    'docs/reference-greedy-frame-oracle.md': '199e1a2b2739b72749cbd0b9d84093dc3d28faeae31a5463b8a03b96f8041a95',
}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def load_inputs():
    for name, digest in PINS.items():
        require(sha(ROOT / name) == digest, 'reviewed input identity: ' + name)
    import check_greedy_frame_recipes as recipes
    import check_greedy_frame_floor_recipes as floor_recipes
    import capture_double_fast_tiny as bounded
    from oracles import greedy_frame_validation as validator
    recipes.check(); floor_recipes.check()
    generator = recipes.load_generator()
    cases = generator.recipes() + floor_recipes.load_generator().recipes()
    require(len(cases) == len({c.name for c in cases}) == 138, 'combined recipe count')
    return recipes, floor_recipes, bounded, validator, generator, cases


def malformed(inputs, frames):
    source = str(inputs / 'chain_first_uniform.source')
    result = []
    def frame(name, change):
        output = frames / ('negative-' + name + '.zst')
        argv = ['frame', 'negative_' + name, source, '4096', str(output), '-', '14', '10', '10', '4', '4', '0']
        change(argv)
        result.append((name, argv, output))
    for name, index, value in [
        ('window_low',6,'9'), ('window_high',6,'22'), ('hash_low',7,'5'), ('hash_high',7,'20'),
        ('chain_low',8,'5'), ('chain_resource',8,'19'), ('search_zero',9,'0'), ('search_resource',9,'9'),
        ('minimum_low',10,'2'), ('minimum_high',10,'8'), ('target_high',11,'131073'),
        ('negative_scalar',7,'-1'), ('float_scalar',7,'6.0'), ('narrowing',7,'4294967296'),
        ('overflow_scalar',7,'18446744073709551616'), ('case_charset',1,'UPPER'),
        ('case_length',1,'x'*64), ('source_missing',2,str(inputs/'absent.source')),
        ('source_oversize',2,str(inputs/'oversize.source')), ('schedule_short',3,'4095'),
        ('schedule_negative',3,'-1'), ('schedule_float',3,'4096.0'), ('schedule_trailing',3,'4096,'),
        ('schedule_many',3,','.join(['0']*65)), ('block_large',3,'131073'), ('window_block',6,'10'),
    ]:
        frame(name, lambda a,index=index,value=value: a.__setitem__(index,value))
    frame('row_chain_high', lambda a: (a.__setitem__(6,'15'), a.__setitem__(8,'30')))
    frame('row_search_high', lambda a: (a.__setitem__(6,'15'), a.__setitem__(9,'30')))
    for name, pledge, window in [('header_unknown','18446744073709551615','21'),
                                  ('header_overflow','18446744073709551616','21'),
                                  ('header_negative','-1','21'), ('header_window_low','1','9'),
                                  ('header_window_high','1','22')]:
        result.append((name,['header',pledge,window],None))
    result.append(('bad_mode',['unknown'],None))
    result.append(('metadata_extra',['metadata','extra'],None))
    return result


def plan():
    _, _, _, _, generator, cases = load_inputs()
    negatives = malformed(Path('/planned-inputs'), Path('/planned-frames'))
    require(len(negatives) == len({n for n,_,_ in negatives}) == 35, 'malformed plan count')
    return {'recipes':len(cases), 'source_bytes':sum(len(c.source) for c in cases),
            'scheduled_calls':sum(len(c.schedule) for c in cases),
            'positive_calls':sum(sum(n > 0 for n in c.schedule) for c in cases),
            'whole_comparisons':sum(c.whole_compare for c in cases),
            'incomplete_headers':len(generator.HEADERS), 'malformed_per_build':len(negatives),
            'codec_executions':0, 'status':'pre-execution plan checked'}


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
    (inputs / 'floor-recipes.json').write_bytes(floor_recipes.MANIFEST.read_bytes())
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
        helper = ROOT / 'scripts/oracles/greedy_frame_v3.c'
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
            executable = build / 'greedy-frame-v3'
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
                frame = frames / (case.name + '.zst')
                whole = frames / (case.name + '.whole.zst') if case.whole_compare else None
                schedule = ','.join(str(n) for n in case.schedule) or '-'
                argv = [str(executable), 'frame', case.name, str(inputs / (case.name + '.source')), schedule,
                        str(frame), str(whole) if whole else '-', str(case.window_log), str(case.hash_log), str(case.chain_log),
                        str(case.search_log), str(case.min_match), str(case.target)]
                path = command(mode + '-' + case.name, argv, overrides)
                require(not (output / (mode + '-' + case.name + '.stderr')).read_bytes(), 'unexpected C/sanitizer diagnostics')
                logical = records(path)
                summary = validator.validate_case(case, logical, frame_bytes(frame, case), frame_bytes(whole, case) if whole else None)
                if mode == 'sanitized':
                    require(path.read_bytes() == (output / ('normal-' + case.name + '.stdout')).read_bytes(), 'complete normal/sanitized observations')
                    for item in [frame] + ([whole] if whole else []):
                        require(frame_bytes(item, case) == frame_bytes(output / 'frames-normal' / item.name, case), 'complete normal/sanitized frame bytes')
                summary.update(stdout_sha256=sha(path), frame_sha256=sha(frame), frame_bytes=frame.stat().st_size,
                               whole_sha256=sha(whole) if whole else None, whole_bytes=whole.stat().st_size if whole else None)
                evidence['results'][mode]['cases'].append(summary); save()
                if index % 5 == 4 or index + 1 == len(cases):
                    print(mode + ': ' + str(index + 1) + '/138 ordinary frame cases', flush=True)
            for index, (pledge, window) in enumerate(generator.HEADERS):
                path = command(mode + '-header-' + str(index), [str(executable), 'header', str(pledge), str(window)], overrides)
                validator.validate_partial(records(path), pledge, window)
                if mode == 'sanitized':
                    require(path.read_bytes() == (output / ('normal-header-' + str(index) + '.stdout')).read_bytes(), 'partial-header build equality')
                evidence['results'][mode]['headers'].append({'pledge': pledge, 'window_log': window, 'stdout_sha256': sha(path)}); save()
            for name, argv, forbidden_output in malformed(inputs, frames):
                require(forbidden_output is None or not forbidden_output.exists(), 'fresh malformed destination')
                path = command(mode + '-negative-' + name, [str(executable)] + argv, overrides, timeout=30, expected_failure=True)
                require(forbidden_output is None or not forbidden_output.exists(), 'malformed command created output')
                evidence['results'][mode]['malformed'].append({'id':name, 'stdout_sha256':sha(path)})
                save()
        require(evidence['results']['normal']['metadata'] == evidence['results']['sanitized']['metadata'], 'native metadata build equality')
        evidence['source_after'] = state(); require(evidence['source_after'] == original, 'final immutable inputs')
        evidence.update(status='passed', finished_at=bounded.utc(), normal_sanitized_equal=True)
        save(); print('Captured 138 ordinary frames, 13 whole comparisons, 15 incomplete headers and 35 malformed rejections in both builds.', flush=True)
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
