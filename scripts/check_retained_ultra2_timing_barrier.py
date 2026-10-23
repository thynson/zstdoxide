"""Pure controls for the timing prerequisite schema; synthetic records are not gate evidence."""
from pathlib import Path
import copy
import datetime
import hashlib
import json
import sys

import benchmark_retained_ultra2_frames_v2 as benchmark

children = []
def guard(event, args):
    if event in ('subprocess.Popen', 'os.system', 'os.fork', 'os.posix_spawn', 'os.exec', 'os.spawn'):
        children.append(event)
        raise ValueError('barrier controls forbid child execution')
sys.addaudithook(guard)
production = {str(p.relative_to(benchmark.ROOT)): benchmark.sha(p)
              for p in sorted((benchmark.ROOT / 'src').rglob('*.rs'))}
common = dict(status='passed', source_unchanged=True, production_source_sha256=production)
records = dict(
    platform=dict(common, interpreted_test_executions=4, processes_remaining=0, concrete_codegen_builds=2),
    matrix=dict(common, isolated_features=19, failed_commands=0, complete_products=22, no_std_builds=21),
    native=dict(common, recipes=1672, failed_commands=0),
    clock_free=dict(status='passed', source_unchanged=True, timed=False, cpu_barrier=None,
                    configs=['safe', *benchmark.FEATURES, 'all'], source=benchmark.source_manifest()),
)
def envelope(values):
    result = dict(schema_version=1, recorded_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  task_owned_jobs_remaining=[], completed={}, qualification='Synthetic schema control only; no completed CPU barrier is asserted.')
    for name, value in values.items():
        text = json.dumps(value, sort_keys=True)
        result['completed'][name] = dict(path='/synthetic/' + name + '.json',
                                        sha256=hashlib.sha256(text.encode()).hexdigest(), json=text)
    return result
baseline = envelope(records)
benchmark.validate_barrier(baseline, True)
checks = []
mutations = [
    ('predecessor-native-count', 'native', 'recipes', 1593),
    ('incomplete-native-count', 'native', 'recipes', 1671),
    ('excess-native-count', 'native', 'recipes', 1673),
    ('boolean-native-count', 'native', 'recipes', True),
    ('native-failure', 'native', 'failed_commands', 1),
    ('unfinished-matrix', 'matrix', 'complete_products', 21),
    ('missing-no-std', 'matrix', 'no_std_builds', 20),
    ('live-platform-process', 'platform', 'processes_remaining', 1),
]
for label, group, field, value in mutations:
    changed = copy.deepcopy(records)
    changed[group][field] = value
    forged = envelope(changed)
    try:
        benchmark.validate_barrier(forged, True)
    except ValueError:
        checks.append(label)
    else:
        raise ValueError('accepted coherent barrier forgery: ' + label)
benchmark.validate_barrier(baseline, True)
if len(checks) != 8 or children:
    raise ValueError('complete child-free control inventory')
print(json.dumps(dict(status='passed', negative_controls=checks, positive_controls=2,
                     child_processes=0, qualification='Synthetic schema-only controls with recomputed hashes; no native, matrix or platform execution is claimed.'), sort_keys=True))
