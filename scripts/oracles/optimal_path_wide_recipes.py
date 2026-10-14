"""Source-only candidates straddling the pinned 4096 path horizon."""
from .optimal_paths_wide import Request


def recipes():
    cases = {}
    for length in (4094,4095,4096):
        cases['path_wide_initial_'+str(length)] = Request(b'a'*4112,((1,((4,length),)),),target=4095,literal_mode=1)
    for end in (4095,4096,4097):
        cases['path_wide_later_'+str(end)] = Request(b'a'*4108,((1,((4,4095),)),(2051,((4,end-2050),))),target=4095,literal_mode=1)
    cases['path_wide_source_end'] = Request(b'a'*4096,((1,((4,4094),)),(2051,((4,2045),))),target=4095,literal_mode=1)
    cases['path_wide_maximum_immediate'] = Request(b'a'*8192,((1,((4,8191),)),),target=4095,literal_mode=1)
    cases['path_wide_strict_target'] = Request(b'a'*4112,((1,((4,4095),)),),target=4094,literal_mode=1)
    cases['path_wide_literal_pricing'] = Request(b'a'*4108,((1,((4,4095),)),(2051,((4,2046),))),target=4095,literal_mode=0)
    return cases
