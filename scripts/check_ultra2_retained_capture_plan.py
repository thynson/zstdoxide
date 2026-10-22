#!/usr/bin/env python3
"""Pure preparation and forged-plan controls for ordinary Ultra capture."""
import argparse
from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys
import ultra2_retained_capture_plan as specification
from capture_ultra2_retained import prepare
from oracles.ultra2_retained import SourceRequest as Request, need as require
from oracles.ultra2_instrumentation import instrument


def check(upstream, output, compiler):
    children = []
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.posix_spawn','os.fork'):
            children.append(event)
            raise ValueError('pure preflight child execution forbidden')
    sys.addaudithook(guard)
    original = subprocess.Popen
    def blocked(*args, **kwargs):
        children.append((args, kwargs))
        raise ValueError('pure preflight attempted a child')
    subprocess.Popen = blocked
    try:
        upstream, output, compiler, source, reference, rows = prepare(upstream, output, compiler)
        _, units = specification.reference_sources(upstream)
        rejected = 0
        forged = [rows[:-1], rows + [rows[-1]], list(reversed(rows))]
        for field, value in [('expected_exit', 1), ('expected_diagnostic', 'forged'), ('timeout_seconds', 999), ('stdout_limit', 1 << 30), ('stderr_limit', 1 << 30), ('label', 'forged')]:
            changed = deepcopy(rows)
            changed[0][field] = value
            forged.append(changed)
        changed = deepcopy(rows)
        row = next(row for row in changed if row['label'] == 'sanitized-compile-0')
        row['argv'].remove('-fsanitize=address,undefined')
        forged.append(changed)
        for changed in forged:
            try: specification.validate_plan(changed, upstream, output, compiler, units)
            except ValueError: rejected += 1
            else: raise ValueError('forged plan accepted')
        raw = (upstream/'lib/compress/zstd_opt.c').read_bytes()
        transformed, manifest = instrument(raw)
        require(len(manifest['substitutions']) == 19 and transformed != raw, 'reviewed unique substitutions')
        anchor = b'ip += (ip==prefixStart);'
        alternative = b'int withMoreLiterals = price + LIT_PRICE(ip+cur) + LL_INCPRICE(litlen+1);'
        for bad in [raw.replace(anchor, b'/* absent */'), raw + b'\n' + anchor, raw.replace(alternative,b'/* absent */'),raw+b'\n'+alternative]:
            try: instrument(bad)
            except ValueError: rejected += 1
            else: raise ValueError('nonunique instrumentation accepted')
        previous=(specification.ROOT/'scripts/oracles/ultra_retained.c').read_bytes()
        current=(specification.ROOT/specification.HELPER).read_bytes()
        specification.adapter_changes(previous,current)
        for before,after in [(b'!'+previous[1:],current),(previous,b'!'+current[1:])]:
            try:specification.adapter_changes(before,after)
            except ValueError:rejected+=1
            else:raise ValueError('unreviewed helper adaptation accepted')
        environment = dict(PATH='/usr/bin:/bin',LANG='C',LC_ALL='C',TZ='UTC',
            ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
        specification.environment(environment)
        for key,value in [('DYLD_INSERT_LIBRARIES','/tmp/injection'),('ASAN_OPTIONS',''),('LANG',True)]:
            changed=dict(environment);changed[key]=value
            try:specification.environment(changed)
            except ValueError:rejected+=1
            else:raise ValueError('unsafe environment accepted')
        for label,out,err in [
            ('reference-revision',b'bad',b''),('reference-status',b' M lib/a.c',b''),
            ('normal-dependencies',b'x: /tmp/injected.h\n',b''),
            ('normal-predefines',b'#define DEBUGLEVEL 0\n',b''),
            ('normal-compile-0',b'',b'warning')]:
            try:specification.command_gate(label,out,err,upstream,output,reference)
            except ValueError:rejected+=1
            else:raise ValueError('bad pre-execution gate accepted')
        from check_ultra2_retained_capture import validate_frame,validate_ledger,validate_ledger_pair
        request=Request(b'a'*128,(64,64))
        block=(64<<3).to_bytes(3,'little')+b'a'*64
        frame=bytes.fromhex('28b52ffd2080')+block+block+bytes((1,0,0))
        validate_frame(frame,request,(0,0))
        validate_ledger(dict(allocations=[16,64],peak_bytes=80,released=True))
        for data in [frame[:-1],frame+b'!',frame[:5]+b'\x3f'+frame[6:],frame[:-1]+b'b']:
            try:validate_frame(data,request,(0,0))
            except ValueError:rejected+=1
            else:raise ValueError('bad complete frame accepted')
        for value in [dict(allocations=[True],peak_bytes=1,released=True),
                      dict(allocations=[16],peak_bytes=15,released=True),
                      dict(allocations=[16],peak_bytes=16,released=1)]:
            try:validate_ledger(value)
            except ValueError:rejected+=1
            else:raise ValueError('bad ledger accepted')
        first=dict(allocations=[100,200],peak_bytes=300,released=True)
        second=dict(allocations=[100,3784],peak_bytes=3884,released=True)
        validate_ledger_pair(first,second)
        for value in [first,dict(second,peak_bytes=3883),dict(second,allocations=[101,3784])]:
            try:validate_ledger_pair(first,value)
            except ValueError:rejected+=1
            else:raise ValueError('bad sanitizer ledger relationship accepted')
        require(not children, 'pure preflight')
        loaded={str(Path(m.__file__).resolve().relative_to(specification.ROOT)):specification.sha(Path(m.__file__).resolve().read_bytes()) for m in list(sys.modules.values()) if getattr(m,'__file__',None) and Path(m.__file__).suffix=='.py' and Path(m.__file__).resolve().is_relative_to(specification.ROOT/'scripts')}
        require(all(source.get(name)==digest for name,digest in loaded.items()),'all runtime local imports belong to the exact source closure')
        return dict(status='passed', optimized=not __debug__, runtime_source_sha256=loaded, commands=len(rows), codec_units=len(units)+1,
                    source_files=len(source), reference_files=len(reference), substitutions=19,
                    negative_packets=len(specification.malformed_packets()), special_inputs=len(specification.special_inputs()), forged_rejected=rejected, child_processes=0)
    finally:
        subprocess.Popen = original


if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--upstream',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--cc',default='clang')
    a=p.parse_args();print(json.dumps(check(a.upstream,a.output,a.cc),sort_keys=True))
