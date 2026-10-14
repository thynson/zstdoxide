#!/usr/bin/env python3
"""Check reviewed observation-only substitutions without compiling or running C."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from oracles.optimal_startup import require
from oracles.optimal_startup_instrumentation import instrument,PATCHES


def check(reference):
    original=(reference/'lib/compress/zstd_opt.c').read_bytes()
    children=[]
    def blocked(*args,**kwargs):children.append(1);raise ValueError('no child processes')
    prior=subprocess.Popen;subprocess.Popen=blocked
    try:
        generated,manifest=instrument(original)
        require(len(manifest['substitutions'])==14,'fourteen observation anchors')
        require(generated.count(b'STARTUP_SEARCH_BEGIN(')==2,'both query snapshots')
        require(generated.count(b'ZSTD_ALLOW_POINTER_OVERFLOW_ATTR')==original.count(b'ZSTD_ALLOW_POINTER_OVERFLOW_ATTR'),'original pointer attributes retained')
        rejected=0
        for forged in [original+b'\n',original.replace(b'cur = 1;',b'cur = 2;',1),original.replace(b'ZSTD_selectBtGetAllMatches(ms, dictMode)',b'observer_matches',1),original.replace(PATCHES[0][1].encode(),b'',1)]:
            require(forged!=original,'meaningful mutation')
            try:instrument(forged)
            except ValueError:rejected+=1
            else:raise RuntimeError('mutated source accepted')
        require(not children,'no children')
        return dict(status='passed',anchors=14,rejected=rejected,child_processes=0,
            original_sha256=manifest['original_sha256'],generated_sha256=manifest['generated_sha256'],
            generated_bytes=len(generated),qualification='Source-only hook identity, not compiled C qualification.')
    finally:subprocess.Popen=prior
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--reference',type=Path,required=True)
    print(json.dumps(check(parser.parse_args().reference),sort_keys=True))
