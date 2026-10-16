"""Add strict inter-block catch-up boundaries and a distinct raw-candidate hypothesis."""
from .optimal_retained_recipes import recipes as previous,Recipe
from .optimal_retained import Request,require
from .optimal_startup_recipes import recipes as fresh


def recipes():
    result=previous()
    # The original retained raw hypothesis actually emitted compressed data.
    # Preserve that immutable predecessor under an accurate successor name.
    old=result.pop('retained_raw_rollback')
    result['retained_short_commit']=Recipe(old.request,(2,2,2))
    for length in [385,386,512]:
        result['retained_gap_'+str(length)]=Recipe(Request(b'a'*(length+64),(length,64)),(2,1))
    data=fresh();raw=bytearray(v^0x5a for v in data['startup_noise'].source[:256])
    raw[128:132]=raw[120:124]
    periodic=data['startup_periodic'].source[:256]
    result['retained_raw_candidate']=Recipe(Request(periodic+bytes(raw)+periodic,(256,256,256)),(2,0,2))
    require(len(result)==16,'sixteen retained successor recipes')
    return result
