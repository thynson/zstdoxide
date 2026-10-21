#!/usr/bin/env python3
"""Recompute complete-frame measurements from validated immutable campaigns."""
import argparse
import csv
import json
import math
import statistics
from pathlib import Path

import benchmark_retained_optimal_frames as benchmark


def summarize(primary, repeat, destination):
    benchmark.require(not destination.exists(), 'fresh report directory')
    campaigns = []
    for path, configurations in ((primary, ['safe', *benchmark.FEATURES, 'all']),
                                 (repeat, ['safe', 'all'])):
        evidence = benchmark.shared.load_json(path / 'evidence.json')
        benchmark.require(evidence['timed'] is True and evidence['configs'] == configurations,
                          'complete primary or independent repeat')
        rows = benchmark.validate(path, evidence)
        campaigns.append((path, evidence, rows))
    first, second = campaigns
    benchmark.require(first[1]['source'] == second[1]['source'], 'same measured sources')
    for mode in ('safe', 'all'):
        for left, right in zip(first[2][mode], second[2][mode], strict=True):
            benchmark.require(benchmark.exact(
                {k: v for k, v in left.items() if k != 'samples'},
                {k: v for k, v in right.items() if k != 'samples'}),
                'identical complete frames and allocator ledgers across repeat')
    detailed, aggregates = [], []
    for index, (path, evidence, configurations) in enumerate(campaigns):
        safe_costs = [statistics.median(s['elapsed_ns'] / s['iterations'] / r['source_bytes']
                                       for s in r['samples']) for r in configurations['safe']]
        for config, rows in configurations.items():
            ratios = []
            for row, safe in zip(rows, safe_costs, strict=True):
                costs = [s['elapsed_ns'] / s['iterations'] / row['source_bytes']
                         for s in row['samples']]
                median = statistics.median(costs)
                ratio = median / safe
                ratios.append(ratio)
                detailed.append(dict(campaign='primary' if index == 0 else 'repeat',
                    config=config, workload=row['workload'], source_bytes=row['source_bytes'],
                    frame_bytes=row['frame_bytes'], minimum_ns_per_byte=min(costs),
                    median_ns_per_byte=median, maximum_ns_per_byte=max(costs),
                    cost_ratio_to_safe=ratio, executable_sha256=evidence['executables'][config]['sha256'],
                    peak_requested_bytes=row['allocation']['peak_requested_bytes'],
                    peak_bytes=row['allocation']['peak_bytes'],
                    identities_created=row['allocation']['identities_created']))
            aggregates.append(dict(campaign='primary' if index == 0 else 'repeat', config=config,
                cases=len(rows), samples=sum(len(r['samples']) for r in rows),
                geometric_mean_cost_ratio_to_safe=math.exp(statistics.mean(math.log(r) for r in ratios)),
                minimum_case_cost_ratio=min(ratios), maximum_case_cost_ratio=max(ratios)))
    benchmark.require(len(detailed) == 598 and sum(r['samples'] for r in aggregates) == 2990,
                      'complete preselected measurement inventory')
    destination.mkdir(parents=True)
    with (destination / 'cases.csv').open('w', newline='') as stream:
        writer = csv.DictWriter(stream, fieldnames=list(detailed[0]))
        writer.writeheader()
        writer.writerows(detailed)
    result = dict(status='passed', source=first[1]['source'], case_rows=len(detailed), samples=2990,
        campaigns=[dict(path=str(p), evidence_sha256=benchmark.sha(p / 'evidence.json'))
                   for p, _, _ in campaigns], aggregates=aggregates,
        qualification='Ratios compare median compression-plus-drop cost per byte with the safe build '
                      'on each identical source in the same campaign; less than one is faster. '
                      'Geometric means weight the 26 preselected cases equally. The repeat is '
                      'independent. CPU barriers exclude task-owned jobs, not unrelated desktop activity.')
    benchmark.write(destination / 'summary.json', result)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('primary', type=Path)
    parser.add_argument('repeat', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    result = summarize(args.primary.resolve(), args.repeat.resolve(), args.output.resolve())
    print(json.dumps({k: v for k, v in result.items() if k != 'source'}, sort_keys=True))
