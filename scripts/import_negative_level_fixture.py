#!/usr/bin/env python3
"""Check the negative-level source-state fixtures offline, or reproduce with C."""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import sys
from types import SimpleNamespace

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / 'tests/upstream/negative-level'
IMPORTER = 'scripts/import_negative_level_fixture.py'
GENERATOR = 'tests/upstream/negative-level/oracle.c.in'
DRIVER = 'tests/upstream/negative-level/capture.py'
COMPARE = 'tests/upstream/negative-level/compare.rs'
SUPPORT = 'tests/upstream/support/mod.rs'
REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
DRIVER_SHA256 = 'c38c64a4887b0a706c310a065a0fb60859233dbee7612c397be405e65c0bf70b'
GENERATOR_SHA256 = '7214d68718f69c022d717e402ca374b3a22b7593ec2bb97987e5180bab0827a0'
COMPARE_SHA256 = '5aff294704b567af3eb506a7acb1b238654d5f9c808c287e61d772382175efe0'
CATALOG_SHA256 = '9f5a6143d9a6bbc68a49614c4d6168482e0000f87667d830f3d4ff9eee9dfd12'
FILES = ('advanced.zst', 'capture.json', 'capture.py', 'compare.rs', 'extraction.json',
         'observations.jsonl', 'simple.zst', 'source.raw')
SCOPE = {'kind': 'exact-extracted-source-state-replay', 'source_bytes': 5242880,
         'input_bytes': 1048576, 'writer_stages': 4, 'seed': 0, 'probability_percent': 50,
         'original_destination_capacity': 5263360, 'dictionary_bytes': 0, 'level': -1,
         'all_compressors_gate': True, 'full_prefix_executed': False,
         'original_assertion': 'non-error and equal compressed sizes', 'rust_numeric_execution': False}


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_driver():
    path = ROOT / DRIVER
    require(path.is_file() and sha(path) == DRIVER_SHA256, 'frozen driver before import')
    spec = importlib.util.spec_from_file_location('zstdoxide_negative_level_capture', path)
    require(spec is not None and spec.loader is not None, 'driver module')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def read_json(driver, path, cap=1 << 20):
    require(path.is_file() and path.stat().st_size <= cap, 'bounded JSON file')
    return driver.decode_json(path.read_text())


def check(collection, driver):
    collection = collection.resolve()
    require(collection.is_dir(), 'missing fixture collection')
    expected = set(FILES) | {'manifest.json', 'README.md', 'NOTICE', 'oracle.c.in'}
    actual = {p.relative_to(collection).as_posix() for p in collection.rglob('*') if p.is_file()}
    require(actual == expected, 'missing or unlisted collection file')
    for name in actual:
        require((collection / name).resolve().is_relative_to(collection), 'fixture path escapes collection')
    manifest = read_json(driver, collection / 'manifest.json')
    require(type(manifest) is dict and type(manifest.get('schema_version')) is int
            and manifest['schema_version'] == 1 and manifest.get('upstream_revision') == REVISION,
            'manifest revision/schema')
    require(driver.exact(manifest.get('coverage'), SCOPE), 'honest capture scope')
    require(manifest.get('generator') == GENERATOR and manifest.get('generator_sha256') == GENERATOR_SHA256
            and sha(ROOT / GENERATOR) == GENERATOR_SHA256 and sha(collection / 'oracle.c.in') == GENERATOR_SHA256,
            'frozen generator template')
    require(manifest.get('importer') == IMPORTER and manifest.get('importer_sha256') == sha(Path(__file__)),
            'current importer identity')
    require(driver.exact(manifest.get('repository_input_sha256'), {
        DRIVER: DRIVER_SHA256, COMPARE: COMPARE_SHA256, SUPPORT: sha(ROOT / SUPPORT)}), 'repository input identities')
    require(sha(ROOT / COMPARE) == COMPARE_SHA256, 'independent Rust comparison identity')
    catalog = []
    for name in FILES:
        path = collection / name
        require(path.stat().st_size <= 1048576, 'bounded fixture file')
        record = {'path': name, 'size': path.stat().st_size, 'sha256': sha(path)}
        if name.endswith('.zst'):
            record['expected_output'] = 'source.raw'
        catalog.append(record)
    digest = hashlib.sha256(json.dumps(catalog, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    require(driver.exact(manifest.get('fixtures'), catalog)
            and digest == CATALOG_SHA256 == manifest.get('fixture_catalog_sha256'), 'frozen complete fixture catalog')
    inventory = read_json(driver, ROOT / 'tests/upstream/manifest.json', 16 << 20)
    require(inventory.get('upstream', {}).get('revision') == REVISION, 'source catalog revision')
    known = {row['path']: row['sha256'] for row in inventory['sources'] + inventory['excluded_sources']}
    pinned = manifest.get('source_sha256')
    require(type(pinned) is dict and {'tests/fuzzer.c', 'programs/datagen.c', 'programs/datagen.h',
                                    'programs/platform.h', 'lib/compress/clevels.h'} <= set(pinned), 'source closure')
    for name, digest in pinned.items():
        require(known.get(name) == digest, 'catalog-pinned source: ' + str(name))
    extraction = read_json(driver, collection / 'extraction.json')
    require(extraction['upstream_revision'] == REVISION and extraction['fuzzer_sha256'] == driver.FUZZER_SHA
            and extraction['datagen_sha256'] == driver.DATAGEN_SHA and extraction['full_prefix_executed'] is False
            and extraction['source_state_only'] is True, 'extraction identity/scope')
    require(len(extraction['spans']) == len(driver.SPANS), 'exact extraction span count')
    template = (collection / 'oracle.c.in').read_text()
    for row, (name, (first, last)) in zip(extraction['spans'], driver.SPANS.items()):
        require(row['source'] == 'tests/fuzzer.c' and row['name'] == name
                and type(row['line_start']) is int and row['line_start'] == first
                and type(row['line_end']) is int and row['line_end'] == last
                and type(row['text']) is str and hashlib.sha256(row['text'].encode()).hexdigest() == row['sha256'],
                'verbatim source span identity')
        text = row['text']
        if name == 'TARGET':
            for anchor, hook in (
                ('            if (ZSTD_isError(cSize_1pass)) goto _output_error;\n',
                 '            save_frame("simple", compressedBuffer, cSize_1pass, CNBuffer, srcSize, decodedBuffer, NULL);\n'),
                ('                if (compressionResult != cSize_1pass) goto _output_error;\n',
                 '                save_frame("advanced", compressedBuffer, compressionResult, CNBuffer, srcSize, decodedBuffer, cctx);\n'),
            ):
                require(text.count(anchor) == 1, 'unchanged target hook anchor')
                text = text.replace(anchor, anchor + hook)
        require(template.count('@' + name + '@') == 1, 'unique generated placeholder')
        template = template.replace('@' + name + '@', text.rstrip('\n'))
    require('@' not in template and hashlib.sha256(template.encode()).hexdigest() == extraction['generated_sha256'],
            'offline exact C regeneration from verified extraction spans')
    observed = driver.validate(collection, collection / 'observations.jsonl', require_stages=False)
    capture = read_json(driver, collection / 'capture.json')
    for name in ('metadata', 'frames', 'source_sha256', 'additional_frame_byte_equality'):
        require(driver.exact(observed[name], capture[name]), 'capture result binding: ' + name)
    require(driver.exact(capture['modes'], ['normal', 'sanitized'])
            and capture['normal_sanitizer_equal'] is True
            and type(capture['rust_bytes_compared']) is int and capture['rust_bytes_compared'] == 22020096,
            'observed independent comparison scope')
    require(driver.exact(manifest.get('capture'), capture), 'capture metadata binding')
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--collection', type=Path, default=COLLECTION)
    parser.add_argument('--upstream', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--report', type=Path)
    parser.add_argument('--cc')
    args = parser.parse_args()
    if args.check:
        require(all(value is None for value in (args.upstream, args.output, args.report, args.cc)),
                '--check rejects regeneration arguments')
    else:
        require(args.upstream is not None and args.output is not None, 'explicit --upstream and fresh --output required')
        require(args.collection.resolve() == COLLECTION.resolve(), 'collection override is offline only')
        output = args.output.resolve()
        protected = (ROOT.resolve(), COLLECTION.resolve(), args.upstream.resolve())
        require(not output.exists(), 'preserve existing output')
        for path in protected:
            require(not output.is_relative_to(path) and not path.is_relative_to(output), 'output overlaps protected source')
        if args.report is not None:
            report = args.report.resolve()
            require(not report.exists(), 'preserve existing report')
            for path in (*protected, output):
                require(not report.is_relative_to(path) and not path.is_relative_to(report), 'report/output/source overlap')
    driver = load_driver()
    manifest = check(args.collection, driver)
    if args.check:
        print('Verified negative-level exact extracted input, two C frames, source spans and provenance without subprocesses.')
        return
    driver.run(SimpleNamespace(upstream=args.upstream, repo=ROOT, output=args.output,
                               cc=args.cc or 'cc', prepare_only=False))
    output = args.output.resolve()
    for mode in ('normal', 'sanitized'):
        for name in ('source.raw', 'simple.zst', 'advanced.zst'):
            require((output / mode / name).read_bytes() == (COLLECTION / name).read_bytes(), 'exact frozen regenerated fixture')
        fresh = driver.validate(output / mode, output / (mode + '-capture.stdout'))
        captured = manifest['capture']
        for name in ('stages', 'frames', 'source_sha256', 'additional_frame_byte_equality'):
            require(driver.exact(fresh[name], captured[name]), 'exact regenerated result: ' + name)
        for name, value in fresh['metadata'].items():
            if name != 'size_t_bytes':
                require(driver.exact(value, captured['metadata'][name]), 'regenerated settings')
    require(driver.exact(read_json(driver, output / 'extraction.json'),
                         read_json(driver, COLLECTION / 'extraction.json')), 'identical extracted C statements')
    report = {'schema_version': 1, 'passed': True, 'fixture_catalog_sha256': CATALOG_SHA256,
              'importer_sha256': sha(Path(__file__)), 'capture_evidence_sha256': sha(output / 'evidence.json'),
              'normal_sanitized_bytes_equal_frozen': True, 'upstream_revision': REVISION}
    driver.save(output / 'fixture-comparison.json', report)
    if args.report is not None:
        args.report.resolve().parent.mkdir(parents=True, exist_ok=True)
        driver.save(args.report.resolve(), report)
    print('Fresh normal/sanitized negative-level capture matches every frozen source/frame byte and observation.')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, KeyError, TypeError) as error:
        print('negative-level fixture check failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
