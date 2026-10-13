#!/usr/bin/env python3
"""Capture reviewed ordinary lazy-family frames with bounded immutable evidence."""

from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import sys
import traceback

ROOT = Path(__file__).resolve().parents[1]
PINS = {'scripts/check_lazy_frame_recipes.py': '0a6ac1e5b21a82582048c722951fdaf1b52faf012cab2f65416ee6b766724db0',
 'scripts/oracles/lazy_frames.py': '7fd553c0c39270e4f12828c500fd33485eb5c6b3701062778816285511390894',
 'tests/fixtures/reference-lazy-frame-recipes/recipes.json': '752155495565f4efa295cc25c94635dd2061d002d3b7537493ff027176b3bd0e',
 'scripts/oracles/lazy_frame_v1.c': 'e37b2407cfc9a35a82dc69ae148848b2f2832d59c96f0c51c9d2b08f91ffa951',
 'scripts/oracles/lazy_frame_validation.py': 'b57f0c166b0c2ce5a4ff1f3c87e87eea969d25bdac93f264073d2545d60d11f3',
 'scripts/check_lazy_frame_validator.py': 'fb1d31b7d768514238454a059665a1deb5174c34c88e2a06e08524fdd2e4a944',
 'scripts/capture_double_fast_tiny.py': '5dea723222db1171e6302411d4d59870e01a09dfd7df74bf47c51cc6978bef19',
 'scripts/oracles/double_fast_direct.py': '5280b94e6e10e34a263b7ae1b31a24026adfd92327767b776e81344362604065',
 'scripts/oracles/fast_frame_publication_support.py': 'c1a205a9b7f897232dd7eb1e177f2a318c42da77ac8348226825732b44b0857a',
 'docs/reference-lazy-frame-helper-review.md': 'e0d2cebd9dbfda6cd55d7bd8aae506e78e1d98179357d12d3d878da027e6d7a0',
 'docs/reference-lazy-frame-oracle.md': '139ca5024421717f0af3f6eb0a37cf11dc749afedbfe0b27e07e6592fa89691f',
 'scripts/oracles/greedy_frame_validation.py': '8e400ccad307b562dd6a41ab02c5d00359d36431cd38a5c51040e08e3903c71f',
 'tests/fixtures/reference-greedy-frame-capture/capture.json': '4a6c6b89452f0c22a7f4d9fe9e45255e48c21ab259cfce62a4981e513e63560e',
 'tests/fixtures/reference-greedy-frame-capture/artifacts.bin': '326bbe16b56ea4f353993707fe45f100804983133ec5f5bda4a9ef3b4bbf758d',
 'scripts/oracles/greedy_frames.py': '360bf5139b7e2a886e9d2359c1ea258317238bd1906cfe9d123be33df7a0ce44',
 'scripts/oracles/greedy_frame_floor_recipes.py': 'e30cb2c60762b333d6e4b510e9c9a5384c43a20e93209e89d52716f099bae2c4',
 'scripts/oracles/greedy_direct.py': '616248428077b4b90b873aaef1282fdf4cea5bf39388e027d882c0aa1a1f9114',
 'scripts/oracles/greedy_families.py': 'b8f5be9467988ef60d1a8b9bbdfe7007c08512bc53753cc4ad2e18818b6a0e1f',
 'scripts/oracles/lazy_direct.py': '9a3e1cdb77e8e3d392cf2a3e6975ca53f69c73ae6e8d70a147e5a3950843314e',
 'scripts/oracles/lazy_families.py': 'f1c392d298ad51ff3b19894679c9dcee9a56f1fe22e6b15503a8bb14d3ca3ce3',
 'scripts/oracles/lazy_tiny.py': '6250504d4d87fd545e4b9dbd12b9e31e61dc132e8297da6587bfe468f205ca49',
 'scripts/check_greedy_frame_recipes.py': '9f743ed73771914f1d2e44cdc811fb9a05a59dc44ce40e19f09bff5489a8a700',
 'docs/evidence/2026-10-13-lazy-frame-helper.json': '89605def6a8c5e1babb06892f3253acb065bb28101ca393d623a26d49b2281a6',
 'docs/evidence/2026-10-13-lazy-frame-validator.json': 'd6b1a44e1e436c4ca455ad3b901cf2e84ece1f107694a1f0929db11b78bc5dbf'}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def load_inputs():
    for name, digest in PINS.items():
        path = ROOT / name
        require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 64 << 20,
                'bounded regular reviewed input: ' + name)
        require(sha(path) == digest, 'reviewed input identity: ' + name)
    import check_lazy_frame_recipes as recipes
    import capture_double_fast_tiny as bounded
    from oracles import lazy_frame_validation as validator, lazy_frames as generator
    recipes.check()
    cases = generator.recipes()
    require(len(cases) == len({c.name for c in cases}) == 510, 'combined recipe count')
    return recipes, None, bounded, validator, generator, cases


def predecessor_artifacts():
    """Extract immutable predecessor extents; no parser output is predicted."""
    _, _, _, validator, _, _ = load_inputs()
    root = ROOT / 'tests/fixtures/reference-greedy-frame-capture'
    manifest = validator.parse_json((root / 'capture.json').read_bytes())
    pack = (root / 'artifacts.bin').read_bytes()
    require(len(pack) == manifest['pack_bytes'] and hashlib.sha256(pack).hexdigest() == manifest['pack_sha256'],
            'predecessor pack identity')
    result = {}; cursor = 0
    for name in sorted(manifest['artifacts']):
        item = manifest['artifacts'][name]
        require(type(item['offset']) is int and item['offset'] == cursor and type(item['bytes']) is int
                and 0 <= item['bytes'] <= len(pack) - cursor, 'predecessor extent')
        raw = pack[cursor:cursor + item['bytes']]; cursor += item['bytes']
        require(hashlib.sha256(raw).hexdigest() == item['sha256'], 'predecessor artifact identity')
        result[name] = raw
    require(cursor == len(pack), 'exhausted predecessor pack')
    return result


def malformed(inputs, frames):
    source = str(inputs / 'd0_chain_first_uniform.source')
    result = []
    def frame(name, change):
        output = frames / ('negative-' + name + '.zst')
        argv = ['frame', 'd0_negative_' + name, source, '4096', str(output), '-', '14', '10', '10', '4', '4', '0']
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
    for _, argv, _ in result:
        if argv[0] in ('frame', 'header'):
            argv.append('0')
    for name, value in [('depth_negative','-1'), ('depth_high','3'), ('depth_bool','true'),
                         ('depth_float','1.0'), ('depth_leading_zero','01'),
                         ('depth_overflow','18446744073709551616'), ('depth_narrowing','4294967296'),
                         ('depth_empty','')]:
        output = frames / ('negative-' + name + '.zst')
        result.append((name, ['frame','d0_negative_'+name,source,'4096',str(output),'-','14','10','10','4','4','0',value], output))
    for name, change in [('depth_prefix', lambda a:a.__setitem__(1,'d1_negative_prefix')),
                         ('depth_missing', lambda a:a.pop()),
                         ('depth_extra', lambda a:a.append('extra'))]:
        output = frames / ('negative-' + name + '.zst')
        argv = ['frame','d0_negative_'+name,source,'4096',str(output),'-','14','10','10','4','4','0','0']
        change(argv);result.append((name,argv,output))
    for name, argv in [('header_depth_negative',['header','1','14','-1']),
                       ('header_depth_high',['header','1','14','3']),
                       ('header_depth_leading_zero',['header','1','14','01']),
                       ('header_depth_missing',['header','1','14']),
                       ('header_depth_extra',['header','1','14','0','extra'])]:
        result.append((name,argv,None))
    return result


def plan():
    _, _, _, _, generator, cases = load_inputs()
    negatives = malformed(Path('/planned-inputs'), Path('/planned-frames'))
    require(len(negatives) == len({n for n,_,_ in negatives}) == 51, 'malformed plan count')
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
    (inputs / 'oversize.source').write_bytes(bytes((4 << 20) + 1))
    protected = [Path(__file__).resolve(), ROOT / 'tests/upstream/manifest.json', recipes.GENERATOR, recipes.MANIFEST,
                 ROOT / 'scripts/oracles/fast_frame_publication_support.py',
                 *[ROOT / name for name in PINS], *[upstream / name for name in sorted(source_map)], *sorted(inputs.iterdir())]
    def state():
        return [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in protected]
    original = state()
    predecessors = predecessor_artifacts()
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
        helper = ROOT / 'scripts/oracles/lazy_frame_v1.c'
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
            executable = build / 'lazy-frame-v1'
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
                        str(case.search_log), str(case.min_match), str(case.target), str(case.depth)]
                path = command(mode + '-' + case.name, argv, overrides)
                require(not (output / (mode + '-' + case.name + '.stderr')).read_bytes(), 'unexpected C/sanitizer diagnostics')
                logical = records(path)
                summary = validator.validate_case(case, logical, frame_bytes(frame, case), frame_bytes(whole, case) if whole else None)
                if case.predecessor is not None:
                    old = case.predecessor
                    old_records = [validator.parse_json(line) for line in predecessors[old + '.stdout'].splitlines()]
                    require(validator.predecessor_records(logical, old) == old_records, 'complete depth-zero predecessor observations')
                    require(frame_bytes(frame, case) == predecessors[old + '.zst'], 'complete depth-zero predecessor frame')
                    if whole:
                        require(frame_bytes(whole, case) == predecessors[old + '.whole.zst'], 'complete depth-zero whole frame')
                if mode == 'sanitized':
                    require(path.read_bytes() == (output / ('normal-' + case.name + '.stdout')).read_bytes(), 'complete normal/sanitized observations')
                    for item in [frame] + ([whole] if whole else []):
                        require(frame_bytes(item, case) == frame_bytes(output / 'frames-normal' / item.name, case), 'complete normal/sanitized frame bytes')
                summary.update(stdout_sha256=sha(path), frame_sha256=sha(frame), frame_bytes=frame.stat().st_size,
                               whole_sha256=sha(whole) if whole else None, whole_bytes=whole.stat().st_size if whole else None)
                evidence['results'][mode]['cases'].append(summary); save()
                if index % 5 == 4 or index + 1 == len(cases):
                    print(mode + ': ' + str(index + 1) + '/510 ordinary frame cases', flush=True)
            for index, (pledge, window, depth) in enumerate(generator.HEADERS):
                path = command(mode + '-header-' + str(index), [str(executable), 'header', str(pledge), str(window), str(depth)], overrides)
                validator.validate_partial(records(path), pledge, window, depth)
                if mode == 'sanitized':
                    require(path.read_bytes() == (output / ('normal-header-' + str(index) + '.stdout')).read_bytes(), 'partial-header build equality')
                evidence['results'][mode]['headers'].append({'pledge': pledge, 'window_log': window, 'depth': depth, 'stdout_sha256': sha(path)}); save()
            for name, argv, forbidden_output in malformed(inputs, frames):
                require(forbidden_output is None or not forbidden_output.exists(), 'fresh malformed destination')
                path = command(mode + '-negative-' + name, [str(executable)] + argv, overrides, timeout=30, expected_failure=True)
                require(forbidden_output is None or not forbidden_output.exists(), 'malformed command created output')
                evidence['results'][mode]['malformed'].append({'id':name, 'stdout_sha256':sha(path)})
                save()
        require(evidence['results']['normal']['metadata'] == evidence['results']['sanitized']['metadata'], 'native metadata build equality')
        evidence['source_after'] = state(); require(evidence['source_after'] == original, 'final immutable inputs')
        evidence.update(status='passed', finished_at=bounded.utc(), normal_sanitized_equal=True)
        save(); print('Captured 510 ordinary frames, 39 whole comparisons, 45 incomplete headers and 51 malformed rejections in both builds.', flush=True)
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
