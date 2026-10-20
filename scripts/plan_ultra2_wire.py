#!/usr/bin/env python3
"""Bound and fingerprint complete source-predicted wire records before C runs."""
import hashlib
import sys

sys.dont_write_bytecode = True
from oracles.ultra2_protocol import Request, MAX_LINE, MAX_OUTPUT, canonical, observations
from oracles.ultra2_recipes import recipes
from oracles.ultra_scalar import need


def check(name):
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn','os.exec','os.spawn'):
            children.append(event);raise RuntimeError('wire prediction worker forbids children')
    sys.addaudithook(guard)
    recipe=recipes()[name];request=Request(recipe.request,recipe.seed)
    size=maximum=count=events=0;digest=hashlib.sha256();last=None
    for row in observations(request):
        encoded=(canonical(row)+'\n').encode();size+=len(encoded);maximum=max(maximum,len(encoded));count+=1
        events+=row['record']=='event';digest.update(encoded);last=row
        need(len(encoded)<=MAX_LINE and size<=MAX_OUTPUT,'bounded complete predicted wire output')
    need(last['record']=='complete' and last['events']==events and not children,'complete source-only trailer')
    return dict(name=name,canonical_bytes=size,maximum_line_bytes=maximum,records=count,
                events=events,canonical_sha256=digest.hexdigest(),child_processes=0,optimized=not __debug__)
