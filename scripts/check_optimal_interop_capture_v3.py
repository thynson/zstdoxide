#!/usr/bin/env python3
"""Version 2: admit the completed plan/state source inventory and replay bounded public-frame responses without running any codec."""
from pathlib import Path
import argparse
import hashlib
import json
import re

REVIEWED = {
  "docs/reference-optimal-interop-protocol.md": "16fa1c6c09dbf0e3bf15d88fadb7b8358dc09054af683a72770d51f63c211b51",
  "docs/reference-optimal-interop-adapter-review.md": "587c32ae6b4dc75fe0eeaa0cf18d3f111746b76c10fb7387aabf5561cd8945c9",
  "examples/reference_greedy_oracle.rs": "b982e84fe2df72dcb4ec0b5131d0fa1f88e6dd7c5c0fe29dc17559841da47f7c",
  "scripts/check_optimal_interop_recipes.py": "c8e3641deba8d48c7b7e89d3782f338aa7229dd6b126abc50a3ae1051b65b4f3",
  "scripts/oracles/optimal_interop.py": "d20ade78c9b541868ee2480ea4b5458c76c5872ed84dfda1212ddf99a22b330a",
  "scripts/oracles/greedy_interop_protocol.py": "cbf5c0763f9c1fbd230f988f2b01c517f6af22f58c4645239303a4c75b66b344",
  "scripts/oracles/reference_greedy_frames.c": "e49b52366148ab6c1ee19f150cdade770c72318b391c7fce482b1dca09f7430d",
  "scripts/capture_optimal_interop.py": "654b322cec77fdbe44f21546a39ec63554f85b2bf6f982f7fa03ef4cfa54b72e"
}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def audit_commands(evidence, records, blob, root, expected_sources, pinned):
    """Check exact commands independently of the controller; no child execution."""
    import math
    import os
    project = Path(records['reference-revision']['cwd'])
    upstream = Path(records['reference-revision']['argv'][2])
    capture = Path(records['normal-link']['argv'][-1]).parents[1]
    cc = evidence['compiler']
    for path in (project, upstream, capture, Path(cc)):
        require(path.is_absolute() and str(path) == os.path.normpath(str(path)), 'canonical absolute command path')
    require(not project.is_relative_to(upstream) and not upstream.is_relative_to(project)
            and all(not capture.is_relative_to(p) and not p.is_relative_to(capture) for p in (project, upstream)),
            'disjoint captured roots')
    require(re.fullmatch('[0-9a-f]{64}', evidence['compiler_sha256'])
            and re.fullmatch('[0-9a-f]{64}', evidence['rustc_sha256']), 'compiler identities')
    environment = evidence['environment']
    required = dict(LANG='C', LC_ALL='C', TZ='UTC', CARGO_NET_OFFLINE='true', CARGO_TERM_COLOR='never',
                    RUSTUP_TOOLCHAIN='nightly-2026-10-08', ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',
                    UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
    require(all(environment.get(k) == v for k, v in required.items())
            and set(environment) <= set(required) | {'PATH', 'HOME', 'TMPDIR', 'RUSTUP_HOME', 'CARGO_HOME', 'DEVELOPER_DIR', 'SDKROOT'}
            and all(type(v) is str for v in environment.values()), 'bounded exact runtime environment')
    require(evidence['schema_version'] == 6 and type(evidence['schema_version']) is int, 'successor evidence schema')
    expected = {
        'reference-revision': ['git', '-C', str(upstream), 'rev-parse', 'HEAD'],
        'reference-status': ['git', '-C', str(upstream), 'status', '--porcelain', '--untracked-files=no'],
        'compiler-version': [cc, '--version'], 'compiler-target': [cc, '-dumpmachine'],
        'rustc-version': ['rustc', '+nightly-2026-10-08', '-vV'],
        'rustc-path': ['rustup', 'which', '--toolchain', 'nightly-2026-10-08', 'rustc'],
    }
    helper = project / 'scripts/oracles/reference_greedy_frames.c'
    common = [cc, '-std=c11', '-g', '-I', str(upstream / 'lib'), '-I', str(upstream / 'lib/common'),
              '-I', str(upstream / 'lib/compress'), '-DZSTD_DISABLE_ASM', '-DZSTD_LEGACY_SUPPORT=0', '-DXXH_NAMESPACE=ZSTD_']
    sources = [str(upstream / name) for name in expected_sources]
    executables = {}
    for mode in ('normal', 'sanitized'):
        build = capture / ('c-' + mode)
        exe = build / 'reference-greedy'
        executables[mode] = str(exe)
        flags = ['-O2'] if mode == 'normal' else ['-O1', '-fsanitize=address,undefined', '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']
        expected[mode + '-dependencies'] = common + flags + ['-MM', str(helper), *sources]
        expected[mode + '-predefines'] = common + flags + ['-dM', '-E', str(helper)]
        objects = [str(build / f'unit-{i}.o') for i in range(26)]
        for i, source in enumerate(sources):
            expected[f'{mode}-compile-{i}'] = common + flags + ['-c', source, '-o', objects[i]]
        expected[mode + '-link'] = common + flags + ['-Wall', '-Wextra', '-Werror', str(helper), *objects, '-o', str(exe)]
        raw = blob(records[mode + '-dependencies'], 'stdout').decode().replace('\\\n', ' ')
        dependency_paths = set()
        for line in raw.splitlines():
            require(':' in line, 'dependency rule')
            dependency_paths.update(os.path.normpath(v) for v in line.split(':', 1)[1].split())
        require(str(helper) in dependency_paths and set(sources) <= dependency_paths, 'complete used dependency closure')
        dependencies = {}
        for name in dependency_paths:
            path = Path(name)
            if path == helper:
                dependencies[name] = REVIEWED['scripts/oracles/reference_greedy_frames.c']
            else:
                require(path.is_relative_to(upstream), 'external pinned dependency')
                relative = str(path.relative_to(upstream))
                require(relative in pinned, 'cataloged dependency')
                dependencies[name] = pinned[relative]
        require(dependencies == evidence['builds'][mode]['dependencies'], 'exact compiler dependency identities')
        defines = blob(records[mode + '-predefines'], 'stdout').decode().splitlines()
        require('#define ZSTD_DISABLE_ASM 1' in defines and '#define ZSTD_LEGACY_SUPPORT 0' in defines
                and '#define XXH_NAMESPACE ZSTD_' in defines
                and not any(line.startswith('#define ZSTD_MULTITHREAD') for line in defines), 'effective disabled controls')
    for mode, flags in (('safe', []), ('all', ['--all-features'])):
        target = capture / ('rust-' + mode)
        executables[mode] = str(target / 'release/examples/reference_greedy_oracle')
        expected['rust-build-' + mode] = ['cargo', '+nightly-2026-10-08', 'build', '--offline', '--locked', '--release',
                    '--example', 'reference_greedy_oracle', '--target-dir', str(target), *flags]
    for mode, original in executables.items():
        path = root / Path(original).relative_to(capture)
        require(path.is_file() and not path.is_symlink() and path.resolve().is_relative_to(root)
                and path.stat().st_size <= 64 << 20, 'bounded regular captured executable')
        require(sha(path.read_bytes()) == evidence['builds'][mode]['executable_sha256'], 'captured executable identity')
    for label, row in records.items():
        if label in expected:
            argv, timeout = expected[label], 180
            require(row['stdin'] is None, 'build/metadata command has no request')
        elif '-negative-' in label:
            mode = label.split('-negative-', 1)[0]
            require(mode in executables, 'known negative adapter')
            argv, timeout = [executables[mode]], 30
        else:
            matched = re.fullmatch(r'case-[0-9]{3,4}-(?:rust-encode-(safe|all)|c-(normal|sanitized)-decode-(?:safe|all)|c-encode-(normal|sanitized)|rust-(safe|all)-decode-(?:normal|sanitized))', label)
            require(matched is not None, 'known exchange label')
            mode = next(value for value in matched.groups() if value)
            argv, timeout = [executables[mode]], 30
        require(row['argv'] == argv and row['cwd'] == str(project)
                and type(row['timeout_seconds']) is int and row['timeout_seconds'] == timeout,
                'exact command invocation: ' + label)
        seconds = row['seconds']
        require(type(seconds) in (int, float) and math.isfinite(seconds) and 0 <= seconds <= timeout + 5,
                'bounded successful command duration')
    require(set(expected) <= set(records), 'all 66 metadata/build commands recorded')



def check(directory):
    import check_optimal_interop_recipes as checker
    manifest = checker.check()
    from oracles import optimal_interop as generator, greedy_interop_protocol as protocol
    root = directory.resolve(strict=True)
    report_path = root / 'evidence.json'
    require(report_path.is_file() and not report_path.is_symlink() and report_path.stat().st_size < 32 << 20,
            'bounded evidence file')
    evidence = checker.decode(report_path.read_bytes())
    require(sha(json.dumps(sorted(evidence['source_before']), sort_keys=True, separators=(',', ':')).encode()) == '2d5c16173b3858bf94f4b2f628c9c831ac74d80770f6e8f5bf8404e5fcaaf232'
            and all(type(v) is str and re.fullmatch('[0-9a-f]{64}', v) for v in evidence['source_before'].values()),
            'complete recorded source closure')
    require(evidence['status'] == 'passed' and evidence['source_before'] == evidence['source_after'],
            'completed immutable source snapshot')
    require(evidence['upstream_revision'] == generator.REVISION and evidence['support_archive_used'] is False,
            'pinned source build without support archive')
    require(all(evidence['source_before'].get(name) == digest for name, digest in REVIEWED.items()),
            'reviewed adapter source identities')
    catalog = checker.decode((checker.ROOT / 'tests/upstream/manifest.json').read_bytes())
    pinned = {r['path']: r['sha256'] for r in catalog['sources'] + catalog['excluded_sources']
              if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    require(evidence['upstream_source_sha256'] == pinned, 'complete pinned C source/header identities')
    require((root / 'recipes.json').read_bytes() == checker.MANIFEST.read_bytes(), 'exact input manifest')
    records = {}
    for row in evidence['commands']:
        label = row['label']
        require(type(label) is str and re.fullmatch('[a-zA-Z0-9_-]+', label) and label not in records,
                'unique bounded command label')
        expected = 2 if '-negative-' in label else 0
        require(type(row['exit_code']) is int and type(row['expected_exit']) is int
                and row['exit_code'] == row['expected_exit'] == expected
                and 'error' not in row, 'command exit/error')
        records[label] = row

    def blob(row, kind):
        item = row[kind]
        suffix = '.bin' if kind == 'stdin' else '.' + kind
        folder = 'requests' if kind == 'stdin' else 'logs'
        require(item['path'] == f"{folder}/{row['label']}{suffix}", 'exact artifact path')
        path = root / item['path']
        cap = protocol.MAX_REQUEST + 1 if kind == 'stdin' else protocol.MAX_RESPONSE if kind == 'stdout' else 2 << 20
        require(type(item['bytes']) is int and 0 <= item['bytes'] <= cap and path.is_file()
                and not path.is_symlink() and path.resolve().is_relative_to(root), 'bounded regular artifact')
        data = path.read_bytes()
        require(len(data) == item['bytes'] and sha(data) == item['sha256'], 'exact artifact identity')
        return data

    for row in records.values():
        blob(row, 'stdout')
        blob(row, 'stderr')
        if row['stdin'] is not None:
            blob(row, 'stdin')
    require(len(records) == 14350, 'exact command count')
    used = set()

    def exchange(label, request, magic):
        row = records[label]
        require(row['expected_exit'] == 0 and blob(row, 'stdin') == request and not blob(row, 'stderr'),
                'exact successful request and quiet result')
        used.add(label)
        return protocol.response(blob(row, 'stdout'), magic)

    upstream = Path(records['reference-revision']['argv'][2])
    project = Path(records['reference-revision']['cwd'])
    require(blob(records['reference-revision'], 'stdout').decode().strip() == generator.REVISION,
            'observed reference revision')
    require(not blob(records['reference-status'], 'stdout'), 'clean reference checkout')
    expected_sources = sorted(name for name in evidence['upstream_source_sha256']
                              if any(name.startswith('lib/' + p + '/') for p in ('common', 'compress', 'decompress'))
                              and name.endswith('.c'))
    require(len(expected_sources) == 26, 'complete C source set')
    audit_commands(evidence, records, blob, root, expected_sources, pinned)
    for mode in ('normal', 'sanitized'):
        build = evidence['builds'][mode]
        require(build['compiled_codec_sources'] == expected_sources and
                build['instrumented_codec_sources'] == (expected_sources if mode == 'sanitized' else [])
                and build['helper_instrumented'] is (mode == 'sanitized')
                and build['assembly'] is False and build['workers'] is False and build['leak_sanitizer'] is False,
                'qualified C build scope')
        require(len(build['dependencies']) >= 26, 'non-system dependency coverage')
        for name, digest in build['dependencies'].items():
            path = Path(name)
            if path == project / 'scripts/oracles/reference_greedy_frames.c':
                require(digest == REVIEWED['scripts/oracles/reference_greedy_frames.c'], 'helper dependency')
            else:
                require(path.is_relative_to(upstream) and pinned.get(str(path.relative_to(upstream))) == digest,
                        'pinned non-system dependency')
        for i, name in enumerate(expected_sources):
            argv = records[f'{mode}-compile-{i}']['argv']
            require(argv[argv.index('-c') + 1] == str(upstream / name), 'actual compiled unit')
            require('-DZSTD_DISABLE_ASM' in argv and '-DZSTD_LEGACY_SUPPORT=0' in argv
                    and '-DZSTD_MULTITHREAD' not in argv, 'explicit build switches')
            require(('-fsanitize=address,undefined' in argv) == (mode == 'sanitized'), 'unit instrumentation')
        link = records[mode + '-link']['argv']
        require(('-fsanitize=address,undefined' in link) == (mode == 'sanitized')
                and not any(word.endswith('.a') for word in link), 'helper instrumentation without archive')
        for i in range(26):
            require(sum(word.endswith(f'/unit-{i}.o') for word in link) == 1, 'each used object linked once')
    negatives = protocol.negative_packets()
    for mode in ('normal', 'sanitized', 'safe', 'all'):
        for name, request in negatives:
            label = mode + '-negative-' + name
            row = records[label]
            require(blob(row, 'stdin') == request and not blob(row, 'stdout')
                    and blob(row, 'stderr').startswith(b'reference Greedy')
                    and not any(token in blob(row, 'stderr') for token in (b'AddressSanitizer', b'UndefinedBehaviorSanitizer', b'runtime error:')),
                    'exact negative observation')
            used.add(label)
    require(evidence['negative_rejections'] == 156, 'all negative rejections')
    import csv
    historical = {}
    for name, prefix in [('greedy.tsv', '')]:
        with (checker.ROOT / 'tests/fixtures/reference-admission-v4' / name).open() as stream:
            for item in csv.DictReader(stream, delimiter='\t'):
                historical[prefix + item['case']] = list(map(int, item['historical_resolved_tuple'].split(',')))
    require(len(historical) == 422, 'complete historical scalar identities')
    cases = generator.recipes()
    require(len(evidence['cases']) == len(cases) == 1269, 'case count')
    frames = {}
    totals = dict(recipes=1269, admitted=0, unsupported=0, resource=0,
                  rust_frames_c_decoded=0, c_frames_rust_decoded=0, decoded_bytes=0)
    for index, (case, observed) in enumerate(zip(cases, evidence['cases'])):
        require(observed['recipe'] == case.metadata(), 'case metadata')
        prefix = f'case-{index:03}'
        results = []
        for mode in ('safe', 'all'):
            status, parameters, frame = exchange(prefix + '-rust-encode-' + mode,
                    protocol.packet(case.source, case.request), b'ZGOR0001')
            require(status == {'success': 0, 'unsupported': 1, 'resource': 2}[case.expected], 'admission')
            limits = ((10, 31), (6, 30), (6, 30), (1, 30), (3, 7), (0, 131072), (1, 9))
            require(all(low <= value <= high for value, (low, high) in zip(parameters, limits)),
                    'resolved scalar bounds before window arithmetic')
            window, chain, table, search, minimum, target, strategy = parameters
            resource = (window > 20 or table > 18) if strategy == 1 else (
                window > 21 or table > 18 or chain > 18) if strategy == 2 else (
                window > 21 or table > 19 or chain > 19 or search > 8) if strategy == 6 else (
                window > 21 or table > 19 or (window <= 14 and (chain > 18 or search > 8)))
            if strategy == 7:
                resource = not (64 <= len(case.source) <= 16384 and len(case.source) <= 1 << window
                    and window <= 16 and chain <= 12 and table <= 12 and search <= 6 and target <= 4095)
            admitted = 1 if strategy not in (1, 2, 3, 4, 5, 6, 7) else 2 * int(resource)
            require(status == admitted, 'independent strategy/resource admission')
            require(parameters == manifest['resolved_tuples'][index], 'independent complete resolved tuple')
            if case.name in historical:
                require(parameters == historical[case.name], 'unchanged historical scalar tuple')
            row = dict(status=case.expected, resolved=parameters, frame_bytes=len(frame), frame_sha256=sha(frame))
            if status == 0:
                row['frame'] = protocol.frame_info(frame, len(case.source))
                require(row['frame']['window_size'] == min(len(case.source), 1 << parameters[0]), 'resolved window')
            require(row == observed['rust'][mode], 'Rust response metadata')
            results.append((status, parameters, frame))
        require(results[0] == results[1], 'safe/all complete equality')
        if case.expected != 'success':
            totals[case.expected] += 1
            require(observed['c'] == {'status': 'not-run-for-rejected-Rust-profile'}, 'explicit rejected C scope')
            continue
        totals['admitted'] += 1
        frames[case.name] = results[0][2]
        c_frames = []
        for mode in ('normal', 'sanitized'):
            for rust_mode, (_, _, frame) in zip(('safe', 'all'), results):
                status, zeros, decoded = exchange(prefix + '-c-' + mode + '-decode-' + rust_mode,
                        protocol.packet(case.source, case.request, frame, 1), b'ZGOC0001')
                require(status == 0 and zeros == [0] * 7 and decoded == case.source, 'C decoded exact Rust source')
                totals['rust_frames_c_decoded'] += 1
                totals['decoded_bytes'] += len(decoded)
            status, zeros, frame = exchange(prefix + '-c-encode-' + mode,
                    protocol.packet(case.source, case.request), b'ZGOC0001')
            require(status == 0 and zeros == [0] * 7, 'C encode response')
            require(observed['c'][mode] == dict(frame_bytes=len(frame), frame_sha256=sha(frame),
                    frame=protocol.frame_info(frame, len(case.source))), 'C response metadata')
            c_frames.append(frame)
            for rust_mode in ('safe', 'all'):
                status, zeros, decoded = exchange(prefix + '-rust-' + rust_mode + '-decode-' + mode,
                        protocol.packet(case.source, case.request, frame, 1), b'ZGOR0001')
                require(status == 0 and zeros == [0] * 7 and decoded == case.source, 'Rust decoded exact C source')
                totals['c_frames_rust_decoded'] += 1
                totals['decoded_bytes'] += len(decoded)
        require(c_frames[0] == c_frames[1], 'normal/sanitized C equality')
    for group in manifest['expected_rust_frame_equivalence']:
        require(all(frames[name] == frames[group[0]] for name in group), 'original equivalence hypothesis')
    require(evidence['totals'] == totals and evidence['equivalence_groups'] == 8, 'complete replay totals')
    require((totals['admitted'], totals['unsupported'], totals['resource'], totals['rust_frames_c_decoded'],
             totals['c_frames_rust_decoded'], totals['decoded_bytes']) == (1159, 27, 83, 4636, 4636, 797537304), 'fixed outcomes')
    require(len(used) == 14284 and all(row['stdin'] is None for name, row in records.items() if name not in used),
            'every protocol command replayed')
    return totals


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(check(args.capture), sort_keys=True))


if __name__ == '__main__':
    main()
