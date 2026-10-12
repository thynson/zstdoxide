#!/usr/bin/env python3
"""Independently replay bounded public-frame responses without running any codec."""
from pathlib import Path
import argparse
import hashlib
import json
import re

REVIEWED = {
    'scripts/oracles/reference_double_fast_frames.c': 'dca0eb63b4eed76324de5d4ae9134b0f7dc7a5a7868479fa7eb9bec310171b2c',
    'examples/reference_double_fast_oracle.rs': 'ed455584173111e8186b41e7eec16645fa6c267828a0935831e74196ba87895b',
    'scripts/oracles/double_fast_interop_protocol.py': 'af9562efe67846530ab364ce6c97388af7f407147040b67e330643517e5ad543',
    'docs/reference-double-fast-interop-adapter-review.md': 'ecbca0212280b1dc228f798b62a8a373ba25e4d5cbcc941f7b33befd249b5acc',
}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def check(directory):
    import check_double_fast_interop_recipes as checker
    manifest = checker.check()
    from oracles import double_fast_interop as generator, double_fast_interop_protocol as protocol
    root = directory.resolve(strict=True)
    report_path = root / 'evidence.json'
    require(report_path.is_file() and not report_path.is_symlink() and report_path.stat().st_size < 16 << 20,
            'bounded evidence file')
    evidence = checker.decode(report_path.read_bytes())
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
    require(len(records) == 1666, 'exact command count')
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
            if path == project / 'scripts/oracles/reference_double_fast_frames.c':
                require(digest == REVIEWED['scripts/oracles/reference_double_fast_frames.c'], 'helper dependency')
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
                    and blob(row, 'stderr').startswith(b'reference DoubleFast'), 'exact negative observation')
            used.add(label)
    require(evidence['negative_rejections'] == 152, 'all negative rejections')
    cases = generator.recipes()
    require(len(evidence['cases']) == len(cases) == 159, 'case count')
    frames = {}
    totals = dict(recipes=159, admitted=0, unsupported=0, resource=0,
                  rust_frames_c_decoded=0, c_frames_rust_decoded=0, decoded_bytes=0)
    for index, (case, observed) in enumerate(zip(cases, evidence['cases'])):
        require(observed['recipe'] == case.metadata(), 'case metadata')
        prefix = f'case-{index:03}'
        results = []
        for mode in ('safe', 'all'):
            status, parameters, frame = exchange(prefix + '-rust-encode-' + mode,
                    protocol.packet(case.source, case.request), b'ZDOR0001')
            require(status == {'success': 0, 'unsupported': 1, 'resource': 2}[case.expected], 'admission')
            limits = ((10, 31), (6, 30), (6, 30), (1, 30), (3, 7), (0, 131072), (1, 9))
            require(all(low <= value <= high for value, (low, high) in zip(parameters, limits)),
                    'resolved scalar bounds before window arithmetic')
            window, chain, table, _, _, _, strategy = parameters
            admitted = 1 if strategy not in (1, 2) else int(
                window > (20 if strategy == 1 else 21) or table > 18 or (strategy == 2 and chain > 18)) * 2
            require(status == admitted, 'independent strategy/resource admission')
            row = dict(status=case.expected, resolved=parameters, frame_bytes=len(frame), frame_sha256=sha(frame))
            if status == 0:
                row['frame'] = protocol.frame_info(frame, len(case.source))
                require(row['frame']['window_size'] <= 1 << parameters[0], 'resolved window')
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
                        protocol.packet(case.source, case.request, frame, 1), b'ZDOC0001')
                require(status == 0 and zeros == [0] * 7 and decoded == case.source, 'C decoded exact Rust source')
                totals['rust_frames_c_decoded'] += 1
                totals['decoded_bytes'] += len(decoded)
            status, zeros, frame = exchange(prefix + '-c-encode-' + mode,
                    protocol.packet(case.source, case.request), b'ZDOC0001')
            require(status == 0 and zeros == [0] * 7, 'C encode response')
            require(observed['c'][mode] == dict(frame_bytes=len(frame), frame_sha256=sha(frame),
                    frame=protocol.frame_info(frame, len(case.source))), 'C response metadata')
            c_frames.append(frame)
            for rust_mode in ('safe', 'all'):
                status, zeros, decoded = exchange(prefix + '-rust-' + rust_mode + '-decode-' + mode,
                        protocol.packet(case.source, case.request, frame, 1), b'ZDOR0001')
                require(status == 0 and zeros == [0] * 7 and decoded == case.source, 'Rust decoded exact C source')
                totals['c_frames_rust_decoded'] += 1
                totals['decoded_bytes'] += len(decoded)
        require(c_frames[0] == c_frames[1], 'normal/sanitized C equality')
    for group in manifest['expected_rust_frame_equivalence']:
        require(all(frames[name] == frames[group[0]] for name in group), 'original equivalence hypothesis')
    require(evidence['totals'] == totals and evidence['equivalence_groups'] == 16, 'complete replay totals')
    require((totals['admitted'], totals['unsupported'], totals['resource'], totals['rust_frames_c_decoded'],
             totals['c_frames_rust_decoded'], totals['decoded_bytes']) == (113, 42, 4, 452, 452, 223677280), 'fixed outcomes')
    require(len(used) == 1600 and all(row['stdin'] is None for name, row in records.items() if name not in used),
            'every protocol command replayed')
    return totals


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(check(args.capture), sort_keys=True))


if __name__ == '__main__':
    main()
