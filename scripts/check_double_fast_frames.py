#!/usr/bin/env python3
"""Verify frozen ordinary DoubleFast frame/state observations without subprocesses."""

from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / 'tests/upstream/double-fast-frames'
CATALOG_SHA256 = 'ae4c678c1a38b39d8e914ef3ab021d2a5f986b98d82756d7f7e8c81d0b3b379d'
PINS = {
    'scripts/oracles/double_fast_frame.c': '90e8a70d27bc79d36443033659918fc5458ad70dc8b8006277e39f04168a22dd',
    'scripts/oracles/double_fast_frame_validation.py': '060ffe4e5d6c552d55eb1114d8220616a8b8ca29f55f9a6321e73bd15518e6a5',
    'scripts/oracles/double_fast_frames.py': 'ac10609d7fe7c0f4983899238dc8a852ca437e35aff42b317599a0286c8ea656',
    'scripts/check_double_fast_frame_recipes.py': '629cc39de22147ba8118390a419b772eb4f566e1f0378c7d84614b3eb4792abd',
    'tests/fixtures/reference-double-fast-frame-recipes/recipes.json': '1ac2fe6ddfe66aef7a72f387f0fb8db6e1109ef137f33c1ef63b8496f5fa0667',
    'scripts/capture_double_fast_frames.py': '8b839c4b66df2badf588cc5d5d653f2965661f3acc97beaf116bcb86eb35b023',
    'scripts/capture_double_fast_tiny.py': '5dea723222db1171e6302411d4d59870e01a09dfd7df74bf47c51cc6978bef19',
    'scripts/oracles/double_fast_direct.py': '5280b94e6e10e34a263b7ae1b31a24026adfd92327767b776e81344362604065',
    'scripts/oracles/fast_frame_publication_support.py': 'c1a205a9b7f897232dd7eb1e177f2a318c42da77ac8348226825732b44b0857a',
    'docs/reference-double-fast-frame-helper-review.md': '35caa682ba6e50b80185e251b16d309e25aba3a6480ff6069954745fdec0a304',
}
COVERAGE = {'recipes': 51, 'source_bytes': 6348944, 'scheduled_calls': 164, 'positive_blocks': 160,
            'candidate_generations': 148, 'block_kinds': {'raw': 88, 'rle': 6, 'compressed': 66},
            'discarded_repeat_witnesses': {'raw': 13, 'rle': 3}, 'whole_comparisons': 7,
            'complete_frame_decodes_per_build': 58, 'decoded_bytes_per_build': 12382352,
            'incomplete_header_probes_per_build': 13, 'collector': False, 'public_parameter_resolution': False}


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load():
    for name, digest in PINS.items():
        require(sha(ROOT / name) == digest, 'frozen input identity: ' + name)
    import check_double_fast_frame_recipes as recipe_check
    from oracles import double_fast_frame_validation as validator
    recipe_check.check(recipe_check.MANIFEST)
    generator = recipe_check.load_generator()
    return validator, generator


def review(cases, records, frames, summaries):
    """Measured consequences, with no claim of intermediate table chronology."""
    require([s['id'] for s in summaries] == [c.name for c in cases], 'ordered summaries')
    kinds = [kind for s in summaries for kind in s['block_kinds']]
    rollback = [w['kind'] for s in summaries for w in s['rollback_witnesses']]
    require([kinds.count(i) for i in range(3)] == [88, 6, 66] and [rollback.count(i) for i in (0, 1)] == [13, 3], 'observed publication cardinality')
    require(sum(s['candidate_generations'] for s in summaries) == 148, 'fresh candidate cardinality')
    by_name = {s['id']: s for s in summaries}
    witnesses = {
        'first_uniform': [2], 'zero_uniform': [2], 'compressed_uniform_repeat': [2, 1, 2],
        'mixed_fallback': [2, 0, 1, 2], 'window_expiry17': [0, 0, 0], 'window_retained19': [0, 0, 2],
        'window_expiry20': [0] * 10, 'window_retained21': [0] * 9 + [2], 'window_expiry21': [0] * 18,
    }
    for name, expected in witnesses.items():
        require(by_name[name]['block_kinds'] == expected, 'frozen block-kind witness: ' + name)
    pairs = []
    def equivalent(left, right, ignored_parameter):
        # Remove only declared input ID/parameter differences; every candidate,
        # table, state owner, frame byte and other field still has to agree.
        def normalized(name):
            result = json.loads(json.dumps(records[name]))
            for r in result:
                r['case'] = 'same-input'
                if r['type'] == 'begin':
                    r['applied'][ignored_parameter] = 0
            return result
        require(normalized(left) == normalized(right) and frames[left] == frames[right], 'complete invariant pair: ' + right)
        pairs.append({'left': left, 'right': right, 'ignored_applied_tuple_index': ignored_parameter})
    for name, parameter in [('literal_target4', 5), ('literal_target_max', 5), ('literal_search29', 3)]:
        equivalent('literal_tree', name, parameter)
    equivalent('short_width3', 'short_width4', 4)
    return {'block_kind_witnesses': witnesses, 'equivalent_complete_pairs': pairs,
            'cases': summaries, 'qualification': 'Ordinary publication and complete final maps, not intermediate store tracing or collector/public-resolution behavior.'}


def check(collection):
    validator, generator = load()
    cases = generator.recipes()
    collection = collection.resolve()
    require(collection.is_dir(), 'missing collection')
    names = {'metadata.stdout'} | {'header-' + str(i) + '.stdout' for i in range(13)}
    for case in cases:
        names |= {case.name + '.stdout', case.name + '.zst'}
        if case.whole_compare:
            names.add(case.name + '.whole.zst')
    actual = set()
    for p in collection.rglob('*'):
        require(p.is_file() and not p.is_symlink(), 'flat regular collection')
        actual.add(p.name)
    require(actual == names | {'manifest.json', 'README.md', 'NOTICE'}, 'exact collection paths')
    manifest_path = collection / 'manifest.json'
    require(manifest_path.stat().st_size <= 2 << 20, 'manifest cap')
    manifest = validator.parse_json(manifest_path.read_bytes())
    validator.shape(manifest, {'schema_version', 'upstream_revision', 'generator', 'generator_sha256', 'importer', 'importer_sha256',
                              'repository_input_sha256', 'source_sha256', 'fixtures', 'fixture_catalog_sha256', 'capture', 'coverage', 'policy'}, 'manifest')
    require(type(manifest['schema_version']) is int and manifest['schema_version'] == 1
            and manifest['upstream_revision'] == generator.REVISION, 'manifest schema/revision')
    require(manifest['generator'] == 'scripts/oracles/double_fast_frame.c'
            and manifest['generator_sha256'] == PINS[manifest['generator']], 'reviewed helper')
    require(manifest['importer'] == 'scripts/check_double_fast_frames.py'
            and manifest['importer_sha256'] == sha(ROOT / manifest['importer']), 'checker identity')
    require(manifest['repository_input_sha256'] == {n: h for n, h in PINS.items() if n != manifest['generator']}, 'complete pinned repository inputs')
    require(json.dumps(manifest['coverage'], sort_keys=True) == json.dumps(COVERAGE, sort_keys=True), 'exact coverage')
    catalog = []
    for name in sorted(names):
        p = collection / name
        require(p.stat().st_size <= 64 << 20, 'bounded artifact')
        catalog.append({'path': name, 'size': p.stat().st_size, 'sha256': sha(p)})
    require(manifest['fixtures'] == catalog, 'artifact catalog')
    digest = hashlib.sha256(json.dumps(catalog, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    require(digest == CATALOG_SHA256 == manifest['fixture_catalog_sha256'], 'independently frozen artifact catalog')
    inventory = validator.parse_json((ROOT / 'tests/upstream/manifest.json').read_bytes())
    source_map = {r['path']: r['sha256'] for r in inventory['sources'] + inventory['excluded_sources']
                  if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    require(manifest['source_sha256'] == source_map, 'complete pinned C/header closure')
    capture = manifest['capture']
    validator.shape(capture, {'status', 'commands', 'normal_sanitized_equal', 'support_archive_used', 'metadata', 'compiled_codec_sources',
                              'instrumented_codec_sources', 'helper_instrumented', 'leak_sanitizer', 'assembly', 'review'}, 'capture')
    require(capture['status'] == 'passed' and type(capture['commands']) is int and capture['commands'] == 192
            and capture['normal_sanitized_equal'] is True and capture['support_archive_used'] is False
            and capture['helper_instrumented'] is True and capture['leak_sanitizer'] is False and capture['assembly'] is False, 'capture scope')
    compiled = sorted(n for n in source_map if Path(n).suffix == '.c' and Path(n).parent.as_posix() in ('lib/common', 'lib/compress', 'lib/decompress'))
    require(len(compiled) == 26 and capture['compiled_codec_sources'] == compiled and capture['instrumented_codec_sources'] == compiled, 'all used C instrumentation')
    def records(name):
        return [validator.parse_json(line) for line in (collection / name).read_bytes().splitlines()]
    metadata = records('metadata.stdout')
    require(len(metadata) == 1 and metadata[0] == capture['metadata'], 'native metadata')
    expected = {'type': 'metadata', 'revision': generator.REVISION, 'strategy': 2, 'pointer_bits': metadata[0].get('pointer_bits'),
                'int_bits': 32, 'u32_bytes': 4, 'source_cap': 4194304, 'block_cap': 131072, 'call_cap': 64, 'leading_bytes': 64,
                'suffix_bytes': 262656, 'guard_bytes': 64, 'scratch_bytes': 2097152}
    require(metadata[0] == expected and type(expected['pointer_bits']) is int and expected['pointer_bits'] in (32, 64), 'checked helper dimensions')
    all_records, frames, summaries = {}, {}, []
    for case in cases:
        logical = records(case.name + '.stdout'); frame = (collection / (case.name + '.zst')).read_bytes()
        whole = (collection / (case.name + '.whole.zst')).read_bytes() if case.whole_compare else None
        summaries.append(validator.validate_case(case, logical, frame, whole))
        all_records[case.name], frames[case.name] = logical, frame
    for i, (pledge, window) in enumerate(generator.HEADERS):
        validator.validate_partial(records('header-' + str(i) + '.stdout'), pledge, window)
    measured = review(cases, all_records, frames, summaries)
    require(capture['review'] == measured, 'complete observed publication review')
    return measured


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', required=True)
    parser.add_argument('--collection', type=Path, default=COLLECTION)
    args = parser.parse_args()
    check(args.collection)
    print('Verified 51 ordinary DoubleFast frames / 160 blocks / both complete tables / 13 incomplete headers; offline only.')


if __name__ == '__main__':
    main()
