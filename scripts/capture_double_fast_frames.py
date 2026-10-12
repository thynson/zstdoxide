#!/usr/bin/env python3
"""Capture reviewed ordinary DoubleFast frames with bounded immutable evidence."""

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
    'scripts/check_double_fast_frame_recipes.py': '629cc39de22147ba8118390a419b772eb4f566e1f0378c7d84614b3eb4792abd',
    'scripts/oracles/double_fast_frame.c': '90e8a70d27bc79d36443033659918fc5458ad70dc8b8006277e39f04168a22dd',
    'scripts/oracles/double_fast_frame_validation.py': '060ffe4e5d6c552d55eb1114d8220616a8b8ca29f55f9a6321e73bd15518e6a5',
    'scripts/capture_double_fast_tiny.py': '5dea723222db1171e6302411d4d59870e01a09dfd7df74bf47c51cc6978bef19',
    'scripts/oracles/double_fast_direct.py': '5280b94e6e10e34a263b7ae1b31a24026adfd92327767b776e81344362604065',
    'docs/reference-double-fast-frame-helper-review.md': '35caa682ba6e50b80185e251b16d309e25aba3a6480ff6069954745fdec0a304',
}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def run(args):
    for name, digest in PINS.items():
        require(sha(ROOT / name) == digest, 'reviewed input identity: ' + name)
    import check_double_fast_frame_recipes as recipes
    import capture_double_fast_tiny as bounded
    from oracles import double_fast_frame_validation as validator
    recipes.check(recipes.MANIFEST)
    generator = recipes.load_generator()
    cases = generator.recipes()
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
    def command(label, argv, overrides=None, timeout=180):
        require(state() == original, 'source changed before ' + label)
        row = bounded.run_command(argv, output, label, environment | (overrides or {}), timeout=timeout)
        evidence['commands'].append(row); save()
        require('error' not in row and row.get('exit_code') == 0, 'failed command: ' + label)
        require(state() == original, 'source changed after ' + label)
        return output / row['stdout']['path']
    def records(path):
        require(path.stat().st_size <= 64 << 20, 'bounded records')
        return [validator.parse_json(line) for line in path.read_bytes().splitlines()]
    save()
    try:
        path = command('reference-revision', ['git', '-C', str(upstream), 'rev-parse', 'HEAD'])
        require(path.read_text().strip() == generator.REVISION, 'actual reference revision')
        path = command('reference-status', ['git', '-C', str(upstream), 'status', '--porcelain', '--untracked-files=no'])
        require(not path.read_bytes(), 'tracked reference changed')
        command('compiler-version', [cc, '--version'])
        command('compiler-target', [cc, '-dumpmachine'])
        helper = ROOT / 'scripts/oracles/double_fast_frame.c'
        common = [cc, '-std=c11', '-g', '-I', str(upstream / 'lib'), '-I', str(upstream / 'lib/common'),
                  '-I', str(upstream / 'lib/compress'), '-DZSTD_DISABLE_ASM', '-DZSTD_LEGACY_SUPPORT=0', '-DXXH_NAMESPACE=ZSTD_']
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
            executable = build / 'double-fast-frame'
            command(mode + '-link', common + flags + ['-Wall', '-Wextra', '-Werror', '-Wno-deprecated-declarations',
                    str(helper)] + [str(p) for p in objects] + ['-o', str(executable)])
            overrides = {'ASAN_OPTIONS': 'detect_leaks=0:halt_on_error=1', 'UBSAN_OPTIONS': 'halt_on_error=1:print_stacktrace=1'} if mode == 'sanitized' else {}
            metadata_path = command(mode + '-metadata', [str(executable), 'metadata'], overrides)
            metadata = records(metadata_path)
            require(len(metadata) == 1 and metadata[0]['strategy'] == 2 and metadata[0]['revision'] == generator.REVISION
                    and metadata[0]['int_bits'] == 32 and metadata[0]['u32_bytes'] == 4, 'native metadata')
            evidence['results'][mode] = {'executable_sha256': sha(executable), 'dependencies': dependency_map,
                'compiled_codec_sources': [str(p.relative_to(upstream)) for p in c_sources],
                'instrumented_codec_sources': [str(p.relative_to(upstream)) for p in c_sources] if mode == 'sanitized' else [],
                'helper_instrumented': mode == 'sanitized', 'leak_sanitizer': False, 'assembly': False,
                'metadata': metadata[0], 'cases': [], 'headers': []}
            save()
            for index, case in enumerate(cases):
                frame = frames / (case.name + '.zst')
                whole = frames / (case.name + '.whole.zst') if case.whole_compare else None
                schedule = ','.join(str(n) for n in case.schedule) or '-'
                argv = [str(executable), 'frame', case.name, str(inputs / (case.name + '.source')), schedule,
                        str(frame), str(whole) if whole else '-', str(case.window_log), str(case.long_log), str(case.short_log),
                        str(case.search_log), str(case.min_match), str(case.target)]
                path = command(mode + '-' + case.name, argv, overrides)
                require(not (output / (mode + '-' + case.name + '.stderr')).read_bytes(), 'unexpected C/sanitizer diagnostics')
                logical = records(path)
                summary = validator.validate_case(case, logical, frame.read_bytes(), whole.read_bytes() if whole else None)
                if mode == 'sanitized':
                    require(path.read_bytes() == (output / ('normal-' + case.name + '.stdout')).read_bytes(), 'complete normal/sanitized observations')
                    for item in [frame] + ([whole] if whole else []):
                        require(item.read_bytes() == (output / 'frames-normal' / item.name).read_bytes(), 'complete normal/sanitized frame bytes')
                summary.update(stdout_sha256=sha(path), frame_sha256=sha(frame), frame_bytes=frame.stat().st_size,
                               whole_sha256=sha(whole) if whole else None, whole_bytes=whole.stat().st_size if whole else None)
                evidence['results'][mode]['cases'].append(summary); save()
                if index % 5 == 4 or index + 1 == len(cases):
                    print(mode + ': ' + str(index + 1) + '/51 ordinary frame cases', flush=True)
            for index, (pledge, window) in enumerate(generator.HEADERS):
                path = command(mode + '-header-' + str(index), [str(executable), 'header', str(pledge), str(window)], overrides)
                validator.validate_partial(records(path), pledge, window)
                if mode == 'sanitized':
                    require(path.read_bytes() == (output / ('normal-header-' + str(index) + '.stdout')).read_bytes(), 'partial-header build equality')
                evidence['results'][mode]['headers'].append({'pledge': pledge, 'window_log': window, 'stdout_sha256': sha(path)}); save()
        require(evidence['results']['normal']['metadata'] == evidence['results']['sanitized']['metadata'], 'native metadata build equality')
        evidence['source_after'] = state(); require(evidence['source_after'] == original, 'final immutable inputs')
        evidence.update(status='passed', finished_at=bounded.utc(), normal_sanitized_equal=True)
        save(); print('Captured all51 ordinary frames, seven whole comparisons and13 incomplete headers in both builds.', flush=True)
    except BaseException:
        evidence.update(status='failed', failure=traceback.format_exc(), finished_at=bounded.utc()); save(); raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cc', default='clang')
    run(parser.parse_args())


if __name__ == '__main__':
    main()
