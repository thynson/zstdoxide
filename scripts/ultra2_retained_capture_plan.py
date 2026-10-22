"""Pure bounded capture plan for the retained upstream Ultra2 wrapper observer."""
from pathlib import Path
import hashlib
import json
import struct
from functools import lru_cache
from oracles.ultra2_retained import REVISION, need as require, parse_packet
from oracles.ultra2_retained_recipes_v2 import recipes as source_recipes

@lru_cache(maxsize=1)
def retained_recipes():
    return source_recipes()

@lru_cache(maxsize=1)
def recipes():
    return {name:recipe.request for name,recipe in retained_recipes().items()}
from plan_ultra2_retained_v2 import manifest
from oracles.ultra2_instrumentation import instrument

ROOT = Path(__file__).resolve().parents[1]
HELPER = 'scripts/oracles/ultra2_retained.c'
MANIFEST = 'tests/fixtures/reference-ultra2-retained-recipes-v2/recipes.json'
# This explicit closure is generated and reviewed before the first native run.
INPUTS = ['docs/evidence/2026-10-22-ultra2-retained-model.json', 'docs/reference-ultra2-retained-adapter-review.md', 'docs/reference-ultra2-retained-design.md', 'docs/reference-ultra2-retained-protocol.md', 'scripts/archive_optimal_retained_frames_capture.py', 'scripts/archive_ultra_fresh_capture.py', 'scripts/audit_optimal_retained_rust_frames.py', 'scripts/audit_ultra_retained_rust_frames.py', 'scripts/audit_upstream_tests.py', 'scripts/benchmark_allocators.py', 'scripts/benchmark_double_fast.py', 'scripts/benchmark_fast.py', 'scripts/benchmark_greedy.py', 'scripts/benchmark_lazy.py', 'scripts/benchmark_optimal_frames.py', 'scripts/benchmark_optimal_parse.py', 'scripts/benchmark_optimal_search.py', 'scripts/benchmark_retained_optimal_frames.py', 'scripts/benchmark_retained_optimal_frames_v2.py', 'scripts/benchmark_retained_ultra_frames.py', 'scripts/benchmark_search.py', 'scripts/benchmark_tree.py', 'scripts/benchmark_tree_v2.py', 'scripts/benchmark_ultra2_frames.py', 'scripts/benchmark_ultra_frames.py', 'scripts/benchmark_ultra_frequencies.py', 'scripts/capture_double_fast_families.py', 'scripts/capture_double_fast_frames.py', 'scripts/capture_double_fast_interop.py', 'scripts/capture_double_fast_tiny.py', 'scripts/capture_greedy_families.py', 'scripts/capture_greedy_frames.py', 'scripts/capture_greedy_frames_v2.py', 'scripts/capture_greedy_interop.py', 'scripts/capture_greedy_tiny.py', 'scripts/capture_lazy_families.py', 'scripts/capture_lazy_frames.py', 'scripts/capture_lazy_interop.py', 'scripts/capture_lazy_tiny.py', 'scripts/capture_optimal_fresh_wide.py', 'scripts/capture_optimal_interop.py', 'scripts/capture_optimal_matches.py', 'scripts/capture_optimal_matches_broad.py', 'scripts/capture_optimal_paths.py', 'scripts/capture_optimal_paths_wide.py', 'scripts/capture_optimal_prices.py', 'scripts/capture_optimal_retained.py', 'scripts/capture_optimal_retained_frames.py', 'scripts/capture_optimal_retained_v2.py', 'scripts/capture_optimal_retained_wide.py', 'scripts/capture_optimal_startup.py', 'scripts/capture_optimal_startup_v2.py', 'scripts/capture_tree_families.py', 'scripts/capture_tree_frames.py', 'scripts/capture_tree_interop.py', 'scripts/capture_tree_tiny.py', 'scripts/capture_ultra2.py', 'scripts/capture_ultra2_interop.py', 'scripts/capture_ultra2_retained.py', 'scripts/capture_ultra_frequencies.py', 'scripts/capture_ultra_fresh.py', 'scripts/capture_ultra_interop.py', 'scripts/capture_ultra_paths.py', 'scripts/capture_ultra_retained.py', 'scripts/capture_ultra_scalars.py', 'scripts/check_doc_examples.py', 'scripts/check_double_fast_families.py', 'scripts/check_double_fast_frame_recipes.py', 'scripts/check_double_fast_frames.py', 'scripts/check_double_fast_hashes.py', 'scripts/check_double_fast_interop_capture.py', 'scripts/check_double_fast_interop_recipes.py', 'scripts/check_double_fast_recipes.py', 'scripts/check_double_fast_tiny.py', 'scripts/check_greedy_family_capture.py', 'scripts/check_greedy_family_recipes.py', 'scripts/check_greedy_frame_capture.py', 'scripts/check_greedy_frame_floor_recipes.py', 'scripts/check_greedy_frame_recipes.py', 'scripts/check_greedy_frame_validator.py', 'scripts/check_greedy_frames.py', 'scripts/check_greedy_interop_capture.py', 'scripts/check_greedy_interop_recipes.py', 'scripts/check_greedy_tiny_capture.py', 'scripts/check_greedy_tiny_recipes.py', 'scripts/check_lazy_family_capture.py', 'scripts/check_lazy_family_fixture.py', 'scripts/check_lazy_family_recipes.py', 'scripts/check_lazy_frame_capture.py', 'scripts/check_lazy_frame_recipes.py', 'scripts/check_lazy_frame_validator.py', 'scripts/check_lazy_frames.py', 'scripts/check_lazy_interop_capture.py', 'scripts/check_lazy_interop_capture_v2.py', 'scripts/check_lazy_interop_recipes.py', 'scripts/check_lazy_tiny_capture.py', 'scripts/check_lazy_tiny_fixture.py', 'scripts/check_lazy_tiny_recipes.py', 'scripts/check_match_search_baseline.py', 'scripts/check_miri_filter.py', 'scripts/check_optimal_benchmark_controls.py', 'scripts/check_optimal_frame_baseline.py', 'scripts/check_optimal_fresh_wide_capture.py', 'scripts/check_optimal_fresh_wide_capture_controls.py', 'scripts/check_optimal_fresh_wide_capture_plan.py', 'scripts/check_optimal_fresh_wide_model.py', 'scripts/check_optimal_interop_capture.py', 'scripts/check_optimal_interop_capture_controls.py', 'scripts/check_optimal_interop_capture_controls_v2.py', 'scripts/check_optimal_interop_capture_v2.py', 'scripts/check_optimal_interop_capture_v3.py', 'scripts/check_optimal_interop_plan.py', 'scripts/check_optimal_interop_plan_v2.py', 'scripts/check_optimal_interop_recipe_controls.py', 'scripts/check_optimal_interop_recipes.py', 'scripts/check_optimal_match_broad_capture.py', 'scripts/check_optimal_match_broad_capture_controls.py', 'scripts/check_optimal_match_broad_capture_plan.py', 'scripts/check_optimal_match_broad_recipes.py', 'scripts/check_optimal_match_broad_validation.py', 'scripts/check_optimal_match_capture.py', 'scripts/check_optimal_match_capture_controls.py', 'scripts/check_optimal_match_capture_controls_v2.py', 'scripts/check_optimal_match_capture_plan.py', 'scripts/check_optimal_match_capture_v2.py', 'scripts/check_optimal_match_recipes.py', 'scripts/check_optimal_match_validation.py', 'scripts/check_optimal_match_validation_v2.py', 'scripts/check_optimal_parse_benchmark.py', 'scripts/check_optimal_path_capture.py', 'scripts/check_optimal_path_capture_controls.py', 'scripts/check_optimal_path_capture_plan.py', 'scripts/check_optimal_path_model.py', 'scripts/check_optimal_path_protocol.py', 'scripts/check_optimal_path_wide_capture.py', 'scripts/check_optimal_path_wide_capture_controls.py', 'scripts/check_optimal_path_wide_capture_plan.py', 'scripts/check_optimal_path_wide_model.py', 'scripts/check_optimal_path_wide_protocol.py', 'scripts/check_optimal_price_capture.py', 'scripts/check_optimal_price_capture_controls.py', 'scripts/check_optimal_price_capture_plan.py', 'scripts/check_optimal_price_capture_v2.py', 'scripts/check_optimal_price_protocol.py', 'scripts/check_optimal_price_protocol_v2.py', 'scripts/check_optimal_price_recipes.py', 'scripts/check_optimal_price_validation.py', 'scripts/check_optimal_retained_capture.py', 'scripts/check_optimal_retained_capture_controls.py', 'scripts/check_optimal_retained_capture_plan.py', 'scripts/check_optimal_retained_capture_plan_v2.py', 'scripts/check_optimal_retained_capture_v2.py', 'scripts/check_optimal_retained_frames_capture.py', 'scripts/check_optimal_retained_frames_capture_controls.py', 'scripts/check_optimal_retained_frames_capture_plan.py', 'scripts/check_optimal_retained_frames_model.py', 'scripts/check_optimal_retained_frames_predecessors.py', 'scripts/check_optimal_retained_frames_recipes.py', 'scripts/check_optimal_retained_inventory.py', 'scripts/check_optimal_retained_model.py', 'scripts/check_optimal_retained_model_v2.py', 'scripts/check_optimal_retained_rust_frames_audit.py', 'scripts/check_optimal_retained_wide_capture.py', 'scripts/check_optimal_retained_wide_capture_controls.py', 'scripts/check_optimal_retained_wide_capture_plan.py', 'scripts/check_optimal_retained_wide_model.py', 'scripts/check_optimal_search_benchmark.py', 'scripts/check_optimal_startup_capture.py', 'scripts/check_optimal_startup_capture_controls.py', 'scripts/check_optimal_startup_capture_plan.py', 'scripts/check_optimal_startup_capture_plan_v2.py', 'scripts/check_optimal_startup_capture_v2.py', 'scripts/check_optimal_startup_instrumentation.py', 'scripts/check_optimal_startup_model.py', 'scripts/check_optimal_timed_controls.py', 'scripts/check_reference_admission_v2.py', 'scripts/check_reference_admission_v3.py', 'scripts/check_reference_admission_v4.py', 'scripts/check_reference_admission_v5.py', 'scripts/check_reference_admission_v6.py', 'scripts/check_reference_admission_v7.py', 'scripts/check_reference_admission_v8.py', 'scripts/check_reference_admission_v8_v2.py', 'scripts/check_reference_double_fast_baseline.py', 'scripts/check_reference_fast_baseline.py', 'scripts/check_reference_fast_interop.py', 'scripts/check_reference_greedy_baseline.py', 'scripts/check_reference_interop_v2.py', 'scripts/check_reference_lazy_baseline.py', 'scripts/check_reference_tree_baseline.py', 'scripts/check_reference_tree_baseline_controls.py', 'scripts/check_retained_optimal_benchmark_controls.py', 'scripts/check_retained_optimal_benchmark_controls_v2.py', 'scripts/check_retained_optimal_frame_baseline.py', 'scripts/check_retained_optimal_timed_controls.py', 'scripts/check_retained_optimal_timed_controls_v2.py', 'scripts/check_retained_ultra_benchmark_controls.py', 'scripts/check_retained_ultra_frame_baseline.py', 'scripts/check_retained_ultra_public_recipes.py', 'scripts/check_retained_ultra_public_recipes_v2.py', 'scripts/check_retained_ultra_public_recipes_v3.py', 'scripts/check_retained_ultra_timed_controls.py', 'scripts/check_tree_benchmark_controls.py', 'scripts/check_tree_capture.py', 'scripts/check_tree_families_capture.py', 'scripts/check_tree_family_recipes.py', 'scripts/check_tree_frame_capture.py', 'scripts/check_tree_frame_capture_controls.py', 'scripts/check_tree_frame_capture_v2.py', 'scripts/check_tree_frame_plan.py', 'scripts/check_tree_frame_recipes.py', 'scripts/check_tree_frame_validator.py', 'scripts/check_tree_frames.py', 'scripts/check_tree_interop_capture.py', 'scripts/check_tree_interop_capture_controls.py', 'scripts/check_tree_interop_preflight.py', 'scripts/check_tree_interop_recipes.py', 'scripts/check_tree_recipes.py', 'scripts/check_tree_validator.py', 'scripts/check_ultra2_benchmark_controls.py', 'scripts/check_ultra2_capture.py', 'scripts/check_ultra2_capture_controls.py', 'scripts/check_ultra2_capture_plan.py', 'scripts/check_ultra2_fixture.py', 'scripts/check_ultra2_frame_baseline.py', 'scripts/check_ultra2_interop_capture.py', 'scripts/check_ultra2_interop_capture_controls.py', 'scripts/check_ultra2_interop_plan.py', 'scripts/check_ultra2_interop_predecessor.py', 'scripts/check_ultra2_interop_recipe_controls.py', 'scripts/check_ultra2_interop_recipes.py', 'scripts/check_ultra2_lifecycle.py', 'scripts/check_ultra2_protocol.py', 'scripts/check_ultra2_recipes.py', 'scripts/check_ultra2_retained_capture.py', 'scripts/check_ultra2_retained_capture_controls.py', 'scripts/check_ultra2_retained_capture_plan.py', 'scripts/check_ultra2_retained_model.py', 'scripts/check_ultra2_retained_model_v2.py', 'scripts/check_ultra2_retained_model_v3.py', 'scripts/check_ultra2_retained_model_v4.py', 'scripts/check_ultra2_retained_recipes.py', 'scripts/check_ultra2_retained_recipes_v2.py', 'scripts/check_ultra2_timed_controls.py', 'scripts/check_ultra_benchmark_controls.py', 'scripts/check_ultra_frame_baseline.py', 'scripts/check_ultra_frequency_benchmark.py', 'scripts/check_ultra_frequency_capture.py', 'scripts/check_ultra_frequency_capture_controls.py', 'scripts/check_ultra_frequency_capture_plan.py', 'scripts/check_ultra_frequency_fixture.py', 'scripts/check_ultra_frequency_model.py', 'scripts/check_ultra_fresh_baseline.py', 'scripts/check_ultra_fresh_capture.py', 'scripts/check_ultra_fresh_capture_controls.py', 'scripts/check_ultra_fresh_capture_controls_v2.py', 'scripts/check_ultra_fresh_capture_plan.py', 'scripts/check_ultra_fresh_capture_v2.py', 'scripts/check_ultra_fresh_fixture.py', 'scripts/check_ultra_fresh_recipes.py', 'scripts/check_ultra_interop_capture.py', 'scripts/check_ultra_interop_capture_controls.py', 'scripts/check_ultra_interop_capture_controls_v2.py', 'scripts/check_ultra_interop_capture_controls_v3.py', 'scripts/check_ultra_interop_capture_v2.py', 'scripts/check_ultra_interop_capture_v3.py', 'scripts/check_ultra_interop_plan.py', 'scripts/check_ultra_interop_predecessor.py', 'scripts/check_ultra_interop_predecessor_v2.py', 'scripts/check_ultra_interop_recipe_controls.py', 'scripts/check_ultra_interop_recipes.py', 'scripts/check_ultra_path_baseline.py', 'scripts/check_ultra_path_capture.py', 'scripts/check_ultra_path_capture_controls.py', 'scripts/check_ultra_path_capture_plan.py', 'scripts/check_ultra_path_fixture.py', 'scripts/check_ultra_path_model.py', 'scripts/check_ultra_path_offline.py', 'scripts/check_ultra_retained_capture.py', 'scripts/check_ultra_retained_capture_controls.py', 'scripts/check_ultra_retained_capture_plan.py', 'scripts/check_ultra_retained_fixture.py', 'scripts/check_ultra_retained_model.py', 'scripts/check_ultra_retained_predecessors.py', 'scripts/check_ultra_retained_recipes.py', 'scripts/check_ultra_retained_rust_frames_audit.py', 'scripts/check_ultra_scalar_capture.py', 'scripts/check_ultra_scalar_capture_controls.py', 'scripts/check_ultra_scalar_capture_plan.py', 'scripts/check_ultra_scalar_fixture.py', 'scripts/check_ultra_scalar_model.py', 'scripts/check_ultra_timed_controls.py', 'scripts/check_upstream_fixtures.py', 'scripts/fetch_upstream_corpora.py', 'scripts/freeze_tree_frames.py', 'scripts/generate_upstream_decodecorpus.py', 'scripts/generate_upstream_dictionaries.py', 'scripts/generate_upstream_entropy.py', 'scripts/generate_upstream_windows.py', 'scripts/import_fast_collector.py', 'scripts/import_fast_frame_publication.py', 'scripts/import_fast_parser_families.py', 'scripts/import_fast_parser_smoke.py', 'scripts/import_match_search_fixtures.py', 'scripts/import_negative_level_fixture.py', 'scripts/import_parameter_fixtures.py', 'scripts/import_reference_fast_fuzz.py', 'scripts/import_upstream_golden.py', 'scripts/import_upstream_regressions.py', 'scripts/import_upstream_testcard.py', 'scripts/optimal_fresh_wide_capture_plan.py', 'scripts/optimal_match_broad_capture_plan.py', 'scripts/optimal_match_capture_plan.py', 'scripts/optimal_path_capture_plan.py', 'scripts/optimal_path_wide_capture_plan.py', 'scripts/optimal_price_capture_plan.py', 'scripts/optimal_retained_capture_plan.py', 'scripts/optimal_retained_capture_plan_v2.py', 'scripts/optimal_retained_frames_capture_plan.py', 'scripts/optimal_retained_wide_capture_plan.py', 'scripts/optimal_startup_capture_plan.py', 'scripts/optimal_startup_capture_plan_v2.py', 'scripts/oracles/double_fast_direct.py', 'scripts/oracles/double_fast_families.py', 'scripts/oracles/double_fast_frame_validation.py', 'scripts/oracles/double_fast_frames.py', 'scripts/oracles/double_fast_interop.py', 'scripts/oracles/double_fast_interop_protocol.py', 'scripts/oracles/fast_direct_families.py', 'scripts/oracles/fast_frame_publication_live.py', 'scripts/oracles/fast_frame_publication_support.py', 'scripts/oracles/greedy_direct.py', 'scripts/oracles/greedy_families.py', 'scripts/oracles/greedy_frame_floor_recipes.py', 'scripts/oracles/greedy_frame_validation.py', 'scripts/oracles/greedy_frames.py', 'scripts/oracles/greedy_interop.py', 'scripts/oracles/greedy_interop_protocol.py', 'scripts/oracles/lazy_direct.py', 'scripts/oracles/lazy_families.py', 'scripts/oracles/lazy_frame_validation.py', 'scripts/oracles/lazy_frames.py', 'scripts/oracles/lazy_interop.py', 'scripts/oracles/lazy_tiny.py', 'scripts/oracles/optimal_fresh_wide.py', 'scripts/oracles/optimal_fresh_wide_recipes.py', 'scripts/oracles/optimal_fresh_wide_validation.py', 'scripts/oracles/optimal_interop.py', 'scripts/oracles/optimal_match_broad_recipes.py', 'scripts/oracles/optimal_match_broad_validation.py', 'scripts/oracles/optimal_match_recipes.py', 'scripts/oracles/optimal_match_validation.py', 'scripts/oracles/optimal_match_validation_v2.py', 'scripts/oracles/optimal_matches.py', 'scripts/oracles/optimal_matches_broad.py', 'scripts/oracles/optimal_path_instrumentation.py', 'scripts/oracles/optimal_path_recipes.py', 'scripts/oracles/optimal_path_validation.py', 'scripts/oracles/optimal_path_wide_recipes.py', 'scripts/oracles/optimal_path_wide_validation.py', 'scripts/oracles/optimal_paths.py', 'scripts/oracles/optimal_paths_wide.py', 'scripts/oracles/optimal_price_recipes.py', 'scripts/oracles/optimal_price_validation.py', 'scripts/oracles/optimal_prices.py', 'scripts/oracles/optimal_prices_v2.py', 'scripts/oracles/optimal_retained.py', 'scripts/oracles/optimal_retained_frames.py', 'scripts/oracles/optimal_retained_frames_recipes.py', 'scripts/oracles/optimal_retained_frames_validation.py', 'scripts/oracles/optimal_retained_recipes.py', 'scripts/oracles/optimal_retained_recipes_v2.py', 'scripts/oracles/optimal_retained_validation.py', 'scripts/oracles/optimal_retained_validation_v2.py', 'scripts/oracles/optimal_retained_wide.py', 'scripts/oracles/optimal_retained_wide_recipes.py', 'scripts/oracles/optimal_retained_wide_validation.py', 'scripts/oracles/optimal_startup.py', 'scripts/oracles/optimal_startup_instrumentation.py', 'scripts/oracles/optimal_startup_recipes.py', 'scripts/oracles/optimal_startup_validation.py', 'scripts/oracles/tree_direct.py', 'scripts/oracles/tree_families.py', 'scripts/oracles/tree_frame_validation.py', 'scripts/oracles/tree_frames.py', 'scripts/oracles/tree_interop.py', 'scripts/oracles/tree_validation.py', 'scripts/oracles/ultra2.c', 'scripts/oracles/ultra2_instrumentation.py', 'scripts/oracles/ultra2_interop.py', 'scripts/oracles/ultra2_lifecycle.py', 'scripts/oracles/ultra2_protocol.py', 'scripts/oracles/ultra2_recipes.py', 'scripts/oracles/ultra2_retained.c', 'scripts/oracles/ultra2_retained.py', 'scripts/oracles/ultra2_retained_adapter_changes.json', 'scripts/oracles/ultra2_retained_recipes.py', 'scripts/oracles/ultra2_retained_recipes_v2.py', 'scripts/oracles/ultra2_retained_validation.py', 'scripts/oracles/ultra2_retained_validation_v2.py', 'scripts/oracles/ultra2_retained_validation_v3.py', 'scripts/oracles/ultra2_retained_validation_v4.py', 'scripts/oracles/ultra2_validation.py', 'scripts/oracles/ultra_frequency.py', 'scripts/oracles/ultra_frequency_recipes.py', 'scripts/oracles/ultra_fresh.py', 'scripts/oracles/ultra_fresh_instrumentation.py', 'scripts/oracles/ultra_fresh_recipes.py', 'scripts/oracles/ultra_fresh_validation.py', 'scripts/oracles/ultra_interop.py', 'scripts/oracles/ultra_path_instrumentation.py', 'scripts/oracles/ultra_path_recipes.py', 'scripts/oracles/ultra_path_validation.py', 'scripts/oracles/ultra_paths.py', 'scripts/oracles/ultra_retained.c', 'scripts/oracles/ultra_retained.py', 'scripts/oracles/ultra_retained_recipes.py', 'scripts/oracles/ultra_retained_validation.py', 'scripts/oracles/ultra_scalar.py', 'scripts/oracles/ultra_scalar_recipes.py', 'scripts/pack_double_fast_frame_trajectories.py', 'scripts/pack_double_fast_parser_fixtures.py', 'scripts/pack_fast_frame_trajectories.py', 'scripts/pack_fast_parser_fixtures.py', 'scripts/pack_greedy_frame_trajectories.py', 'scripts/pack_greedy_parser_fixtures.py', 'scripts/pack_lazy_frame_trajectories.py', 'scripts/pack_lazy_parser_fixtures.py', 'scripts/pack_optimal_frame_baseline.py', 'scripts/pack_optimal_frame_baseline_v2.py', 'scripts/pack_optimal_fresh_wide_fixtures.py', 'scripts/pack_optimal_match_broad_fixtures.py', 'scripts/pack_optimal_match_fixtures.py', 'scripts/pack_optimal_path_fixtures.py', 'scripts/pack_optimal_price_fixtures.py', 'scripts/pack_optimal_retained_fixtures.py', 'scripts/pack_optimal_retained_frames_fixtures.py', 'scripts/pack_optimal_retained_frames_fixtures_v2.py', 'scripts/pack_optimal_retained_wide_fixtures.py', 'scripts/pack_optimal_startup_fixtures.py', 'scripts/pack_tree_fixtures.py', 'scripts/pack_tree_frame_trajectories.py', 'scripts/pack_ultra2_fixture.py', 'scripts/pack_ultra2_frame_baseline.py', 'scripts/pack_ultra_frame_baseline.py', 'scripts/pack_ultra_frequency_fixture.py', 'scripts/pack_ultra_fresh_fixture.py', 'scripts/pack_ultra_path_fixture.py', 'scripts/pack_ultra_retained_fixture.py', 'scripts/pack_ultra_scalar_fixture.py', 'scripts/plan_optimal_fresh_wide.py', 'scripts/plan_optimal_retained.py', 'scripts/plan_optimal_retained_frames.py', 'scripts/plan_optimal_retained_v2.py', 'scripts/plan_optimal_retained_wide.py', 'scripts/plan_optimal_startup.py', 'scripts/plan_retained_optimal_public_v3.py', 'scripts/plan_retained_ultra_public.py', 'scripts/plan_retained_ultra_public_v2.py', 'scripts/plan_retained_ultra_public_v3.py', 'scripts/plan_ultra2.py', 'scripts/plan_ultra2_retained.py', 'scripts/plan_ultra2_retained_v2.py', 'scripts/plan_ultra2_wire.py', 'scripts/plan_ultra_fresh.py', 'scripts/plan_ultra_retained.py', 'scripts/project_retained_optimal_frame_baseline.py', 'scripts/project_retained_ultra_frame_baseline.py', 'scripts/repack_tree_frame_evidence.py', 'scripts/summarize_retained_optimal_frames.py', 'scripts/summarize_retained_optimal_frames_v2.py', 'scripts/summarize_retained_ultra_frames.py', 'scripts/summarize_ultra2_frames.py', 'scripts/summarize_ultra_frames.py', 'scripts/tree_capture_inputs.py', 'scripts/tree_family_capture_inputs.py', 'scripts/ultra2_capture_plan.py', 'scripts/ultra2_retained_capture_plan.py', 'scripts/ultra_frequency_capture_plan.py', 'scripts/ultra_fresh_capture_plan.py', 'scripts/ultra_path_capture_plan.py', 'scripts/ultra_retained_capture_plan.py', 'scripts/ultra_scalar_capture_plan.py', 'tests/fixtures/reference-ultra-retained-recipes/recipes.json', 'tests/fixtures/reference-ultra2-retained-recipes-v2/recipes.json', 'tests/fixtures/reference-ultra2-retained-recipes/recipes.json', 'tests/upstream/manifest.json']
MANIFEST_SHA = 'f74739f028307a2d42cd09b67321fd459bed50ef01f9aab74e4cdde9e64b61f9'
ADAPTER_CHANGES = 23



def sha(data):
    return hashlib.sha256(data).hexdigest()


@lru_cache(maxsize=1)
def manifest_bytes():
    return (json.dumps(manifest(),sort_keys=True,indent=2)+'\n').encode()


def source_manifest():
    result = {}
    for name in INPUTS:
        p = ROOT / name
        require(p.is_file() and not p.is_symlink(), 'regular source input: ' + name)
        result[name] = sha(p.read_bytes())
    qualified_model=json.loads((ROOT/'docs/evidence/2026-10-22-ultra2-retained-model.json').read_bytes())
    require(qualified_model['status']=='passed' and all(sha((ROOT/name).read_bytes())==digest for name,digest in qualified_model['source'].items()), 'unchanged independently qualified model inputs')
    manifest_data=(ROOT/MANIFEST).read_bytes()
    require(sha(manifest_data)==MANIFEST_SHA,'immutable pre-execution source hypotheses')
    predictions = json.loads(manifest_data)['cases']
    require(2*sum(r['observation_output_bound'] for r in predictions)<=2<<30 and max(r['observation_output_bound'] for r in predictions)<=256<<20, 'bounded source-derived output inventory')
    cases = retained_recipes()
    require(len(predictions)==len(cases)==32 and {r['name'] for r in predictions}==set(cases), 'complete prediction inventory')
    for row in predictions:
        recipe = cases[row['name']]
        require(row['packet_sha256']==sha(recipe.request.packet())
                and row['hypothesized_kinds']==list(recipe.kinds), 'prediction request identity')
    require(result['tests/upstream/manifest.json']=='3bb5ac1c2a0407b71c5e5dbaabac6b39074f5b7f851801a2056856a910f3ccaa','pinned source inventory')
    return result


@lru_cache(maxsize=1)
def malformed_packets():
    base = recipes()['u2r_wide_frames_uniform_32768'].packet()
    short = recipes()['u2r_alternatives_0202'].packet()
    result = {'short':base[:73], 'oversize':base+b'!', 'truncated':base[:-1],
              'trailing':short+b'!', 'magic':b'!'+base[1:]}
    for name, offset, value in [('bootstrap-disabled',16,0),('bootstrap-invalid',16,2),
        ('policy-zero',20,0),('policy-one',20,1),('policy-max',20,0xffffffff),
        ('short-source',8,13),('source-cap',8,32769),('count-low',12,1),('count-high',12,5),
        ('window-low',24,9),('window-high',24,17),('window-expiry',24,14),
        ('hash-low',28,5),('hash-high',28,13),('chain-low',32,5),('chain-high',32,13),
        ('search-low',36,0),('search-high',36,7),('minimum-low',40,2),('minimum-high',40,8),
        ('target',44,4096),('mode',48,2),('block-low',52,6),('block-high',52,8193),('schedule-total',52,65)]:
        result[name] = base[:offset]+struct.pack('<I',value)+base[offset+4:]
    for name,data in result.items():
        try:r=parse_packet(data)
        except ValueError:pass
        else:require(not r.bootstrap or r.policy!=2 or len(r.original.blocks)<2,
                     'negative request outside the declared native scope')
    return result


def diagnostic(name):
    reason = {'short':'packet size', 'oversize':'packet size',
        'truncated':'complete retained source extent', 'trailing':'complete retained source extent',
        'magic':'packet magic', 'bootstrap-disabled':'native bootstrap scope',
        'bootstrap-invalid':'native bootstrap scope', 'policy-zero':'native bootstrap scope',
        'policy-one':'native bootstrap scope', 'policy-max':'native bootstrap scope',
        'window-expiry':'source fits retained window', 'block-low':'block length',
        'block-high':'block length', 'schedule-total':'exact schedule total',
        'fs-symlink':'packet open', 'fs-missing':'packet open',
        'fs-directory':'regular packet', 'fs-fifo':'regular packet'}.get(name,'request admission')
    require(name in malformed_packets() or name in special_inputs(), 'known negative diagnostic')
    return 'Retained Ultra2 observer: '+reason+'\n'


def special_inputs():
    return ('fs-symlink','fs-missing','fs-directory','fs-fifo')


@lru_cache(maxsize=1)
def requests():
    return {**{name:r.packet()for name,r in recipes().items()},
            **{'negative-'+name:p for name,p in malformed_packets().items()}}


def reference_sources(upstream):
    catalog = json.loads((ROOT / 'tests/upstream/manifest.json').read_bytes())
    require(catalog['upstream']['revision'] == REVISION, 'reference revision')
    sources = {r['path']: r['sha256'] for r in catalog['sources'] + catalog['excluded_sources']
               if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    for name, digest in sources.items():
        p = upstream / name
        require(p.is_file() and not p.is_symlink() and sha(p.read_bytes()) == digest, 'pinned reference source: ' + name)
    units = sorted(str(p.relative_to(upstream)) for folder in ('common', 'compress', 'decompress')
                   for p in (upstream / 'lib' / folder).glob('*.c'))
    require(len(units) == 26 and all(p in sources for p in units), '26 used codec units')
    return sources, [p for p in units if p != 'lib/compress/zstd_opt.c']


def adapter_changes(previous,current):
    changes=json.loads((ROOT/'scripts/oracles/ultra2_retained_adapter_changes.json').read_bytes())
    require(sha(previous)==changes['base_sha256'] and sha(current)==changes['draft_sha256'], 'exact reviewed helper identities')
    require(len(changes['substitutions'])==ADAPTER_CHANGES,'exact reviewed helper adaptations')
    restored=current.decode()
    for patch in reversed(changes['substitutions']):
        require(restored.count(patch['after'])==patch['occurrences'],'unique inverse helper adaptation')
        restored=restored.replace(patch['after'],patch['before'])
    require(restored.encode()==previous,'exact original helper after removing adaptations')
    return changes


def generated(upstream):
    original=(upstream/'lib/compress/zstd_opt.c').read_bytes()
    transformed,changes=instrument(original)
    import difflib
    previous=(ROOT/'scripts/oracles/ultra_retained.c').read_text()
    current=(ROOT/HELPER).read_text()
    adaptations=adapter_changes(previous.encode(),current.encode())
    diff=''.join(difflib.unified_diff(previous.splitlines(True),current.splitlines(True),fromfile='ultra_retained.c',tofile='ultra2_retained.c')).encode()
    return {'adapter-changes.json':(json.dumps(adaptations,sort_keys=True,indent=2)+'\n').encode(),'adapter-diff.patch':diff, 'adapter-digests.json':(json.dumps(dict(previous_sha256=sha(previous.encode()),current_sha256=sha(current.encode())),sort_keys=True)+'\n').encode(), 'original_zstd_opt.c':original,'observed_zstd_opt.c':transformed,
            'instrumentation.json':(json.dumps(changes,sort_keys=True,indent=2)+'\n').encode()}


def artifact_paths(mode, name):
    return [f'artifacts/{mode}/{name}.{suffix}' for suffix in ('0.zst','1.zst','0.json','1.json')]


def plan(upstream,output,compiler,units):
    common=[str(compiler),'-std=c11','-g','-DDEBUGLEVEL=2','-DZSTD_DISABLE_ASM','-DZSTD_LEGACY_SUPPORT=0',
        '-UZSTD_MULTITHREAD','-DXXH_NAMESPACE=ZSTD_','-I',str(output/'generated'),'-I',str(upstream/'lib'),
        '-I',str(upstream/'lib/common'),'-I',str(upstream/'lib/compress')]
    rows=[]
    def add(label,argv,expected=0,expected_diagnostic=None):
        rows.append(dict(label=label,argv=[str(v)for v in argv],expected_exit=expected,expected_diagnostic=expected_diagnostic,
            timeout_seconds=120,stdout_limit=(256<<20)if '-run-u2r_'in label else(2<<20),stderr_limit=1<<20))
    add('reference-revision',['git','-C',upstream,'rev-parse','HEAD'])
    add('reference-status',['git','-C',upstream,'status','--porcelain','--untracked-files=no'])
    add('compiler-version',[compiler,'--version']);add('compiler-target',[compiler,'-dumpmachine'])
    for mode in ['normal','sanitized']:
        flags=['-O2']if mode=='normal'else['-O1','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer']
        base=common+flags;helper=ROOT/HELPER
        add(mode+'-dependencies',base+['-MM',helper,*[upstream/p for p in units]])
        add(mode+'-predefines',base+['-dM','-E',helper]);objects=[]
        for i,unit in enumerate([*[upstream/p for p in units],helper]):
            obj=output/('c-'+mode)/f'unit-{i}.o'
            add(mode+f'-compile-{i}',base+(['-Wall','-Wextra','-Werror']if unit==helper else[])+['-c',unit,'-o',obj]);objects.append(obj)
        executable=output/('c-'+mode)/'ultra2-retained'
        add(mode+'-link',base+[*objects,'-o',executable])
        for name in requests():
            add(mode+'-run-'+name,[executable,output/'requests'/(name+'.bin'),
                *[output/path for path in artifact_paths(mode,name)]],1 if name.startswith('negative-')else 0, diagnostic(name[9:]) if name.startswith('negative-') else None)
        for name in special_inputs():
            add(mode+'-run-negative-'+name,[executable,output/'special'/name,
                *[output/path for path in artifact_paths(mode,'negative-'+name)]],1,diagnostic(name))
    require(len({r['label']for r in rows})==len(rows),'unique commands')
    return rows


def validate_plan(rows,upstream,output,compiler,units):
    require(json.dumps(rows,sort_keys=True)==json.dumps(plan(upstream,output,compiler,units),sort_keys=True),'exact bounded command inventory')
    return len(rows)


def environment(env):
    require(type(env) is dict and set(env) <= {'PATH','HOME','TMPDIR','DEVELOPER_DIR','SDKROOT',
        'LANG','LC_ALL','TZ','ASAN_OPTIONS','UBSAN_OPTIONS'}, 'execution environment closure')
    require(all(type(k) is str and type(v) is str for k,v in env.items()), 'environment strings')
    for key,value in {'LANG':'C','LC_ALL':'C','TZ':'UTC','ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1',
                      'UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'}.items():
        require(env.get(key)==value,'required environment: '+key)


def command_gate(label, out, err, upstream, output, sources):
    # Called before subsequent commands, so an invalid dependency/predefine
    # observation cannot be followed by execution of an unqualified helper.
    require(not err, 'successful command stderr: '+label)
    if label == 'reference-revision':
        require(out.decode().strip()==REVISION,'actual pinned revision')
    elif label == 'reference-status':
        require(not out,'clean reference')
    elif label.endswith('-dependencies'):
        dependencies=set()
        for line in out.decode().replace('\\\n',' ').splitlines():
            require(':' in line,'dependency grammar')
            for text in line.split(':',1)[1].split():
                p=Path(text).resolve()
                require(p==ROOT/HELPER or p==output/'generated/observed_zstd_opt.c' or
                    p.is_relative_to(upstream) and str(p.relative_to(upstream)) in sources,
                    'pinned dependency closure')
                dependencies.add(p)
        require(output/'generated/observed_zstd_opt.c' in dependencies and ROOT/HELPER in dependencies,
                'embedded observer dependency')
    elif label.endswith('-predefines'):
        require(b'#define ZSTD_MULTITHREAD ' not in out and b'#define ZSTD_DISABLE_ASM' in out
                and b'#define DEBUGLEVEL 2' in out,'effective compilation switches')
    elif '-compile-' in label or label.endswith('-link'):
        require(not out,'quiet compilation')

    if label.endswith('-predefines'):
        expected=1 if label.startswith('sanitized-') else 0
        require(f'#define ZSTD_ADDRESS_SANITIZER {expected}\n'.encode() in out,
                'effective workspace sanitizer configuration')
        require(b'#define ZSTD_CWKSP_ASAN_REDZONE_SIZE 128\n' in out and
                b'#define ZSTD_ASAN_DONT_POISON_WORKSPACE' not in out,'original workspace redzones')
