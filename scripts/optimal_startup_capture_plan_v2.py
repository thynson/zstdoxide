"""Successor capture plan retaining ordinary workspace sanitizer redzones.

The C helper, packets, hooks and exact command inventory are unchanged. Only
allocator comparison and effective sanitizer checks differ from v1.
"""
from optimal_startup_capture_plan import (
    ROOT, HELPER, MANIFEST, REVISION, require, sha, recipes, malformed_packets,
    requests, reference_sources, generated, plan, validate_plan, artifact_paths,
    environment, source_manifest as original_manifest, command_gate as original_gate,
)


def source_manifest():
    result=original_manifest()
    for name in ['scripts/optimal_startup_capture_plan_v2.py','scripts/capture_optimal_startup_v2.py',
                 'scripts/check_optimal_startup_capture_v2.py','scripts/check_optimal_startup_capture_plan_v2.py',
                 'docs/reference-optimal-startup-capture-v2.md']:
        p=ROOT/name
        require(p.is_file() and not p.is_symlink(),'regular successor source')
        result[name]=sha(p.read_bytes())
    return result


def command_gate(label,out,err,upstream,output,sources):
    original_gate(label,out,err,upstream,output,sources)
    if label.endswith('-predefines'):
        expected=1 if label.startswith('sanitized-') else 0
        require(f'#define ZSTD_ADDRESS_SANITIZER {expected}\n'.encode() in out,
                'effective workspace sanitizer configuration')
        require(b'#define ZSTD_CWKSP_ASAN_REDZONE_SIZE 128\n' in out and
                b'#define ZSTD_ASAN_DONT_POISON_WORKSPACE' not in out,'original workspace redzones')
