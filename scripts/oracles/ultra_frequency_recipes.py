"""Source-only coupled-policy wrappers preserve every predecessor request."""
from . import optimal_price_recipes as old
from .optimal_prices_v2 import Request as Original
from . import ultra_frequency as m
from .ultra_scalar import sha


def recipes():
    result=[]
    for policy in (0,2):
        for previous in old.recipes():
            result.append((f'uf_p{policy}_'+previous.name[3:],m.Request(policy,previous.request),previous.name))
        source=b'\0\1'*65536
        for extra in (29,30,31):
            operations=((0,0,9,0,0,0),(5,0,131071,0xffffffff,131072,32),(1,0,extra,1,3,0),
                        (2,0,0,0,0,0),(4,0,256,131072,1<<20,131072),(3,0,0,0,0,0),(4,0,256,131072,1<<20,131072))
            result.append((f'uf_p{policy}_fraction_boundary_{extra}',m.Request(policy,Original(source,operations,0)),None))
    m.need(len(result)==300 and len({name for name,_,_ in result})==300,'unique source-only recipes')
    return result


def manifest():
    rows=[]
    for name,request,previous in recipes():
        try:request.validate()
        except ValueError as error:accepted=False;reason=str(error)
        else:accepted=True;reason=None
        original=request.original
        rows.append(dict(id=name,original_id=previous,policy=request.policy,literal_mode=original.literal_mode,
                         expected_exit=0 if accepted else 1,rejection=reason,source_bytes=len(original.source),
                         source_sha256=sha(original.source),original_packet_sha256=sha(original.packet()),
                         packet_sha256=sha(request.raw_packet()),operations=[list(op)for op in original.operations]))
    return dict(schema_version=1,revision=m.REVISION,recipes=rows,qualification='Source-only policy wrappers and exact-boundary requests. Original inputs are unchanged, and fractional observation-envelope rejections are explicit before C execution.')
