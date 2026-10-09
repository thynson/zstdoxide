/* Pinned facebook/zstd v1.5.7 parameter oracle, test infrastructure only.
 * Derived clamp/table references retain upstream BSD-3-Clause attribution;
 * see root LICENSE.zstd and this directory's README.md. Never linked by Cargo.
 */
#define ZSTD_STATIC_LINKING_ONLY
#include "zstd.h"
#include "zstd_errors.h"
#include "compress/zstd_compress_internal.h"
#include "compress/clevels.h"
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define UNKNOWN ZSTD_CONTENTSIZE_UNKNOWN
#define CP_HEADER "window_log\tchain_log\thash_log\tsearch_log\tmin_match\ttarget_length\tstrategy"
static const ZSTD_cParameter fields[7] = { ZSTD_c_windowLog, ZSTD_c_chainLog,
    ZSTD_c_hashLog, ZSTD_c_searchLog, ZSTD_c_minMatch, ZSTD_c_targetLength, ZSTD_c_strategy };
static const char* names[7] = { "window_log", "chain_log", "hash_log", "search_log",
    "min_match", "target_length", "strategy" };
static const ZSTD_cParameter requests[12] = { ZSTD_c_compressionLevel, ZSTD_c_windowLog,
    ZSTD_c_chainLog, ZSTD_c_hashLog, ZSTD_c_searchLog, ZSTD_c_minMatch, ZSTD_c_targetLength,
    ZSTD_c_strategy, ZSTD_c_srcSizeHint, ZSTD_c_contentSizeFlag, ZSTD_c_checksumFlag, ZSTD_c_dictIDFlag };
static const char* request_names[12] = { "level", "window_log", "chain_log", "hash_log",
    "search_log", "min_match", "target_length", "strategy", "source_hint", "content_size",
    "checksum", "dictionary_id" };
static void require(int condition, const char* reason) {
    if (!condition) { fprintf(stderr, "%s\n", reason); exit(1); }
}
static void okay(size_t code) { require(!ZSTD_isError(code), ZSTD_getErrorName(code)); }
static void cp(ZSTD_compressionParameters p) {
    printf("%u\t%u\t%u\t%u\t%u\t%u\t%u", p.windowLog, p.chainLog, p.hashLog,
           p.searchLog, p.minMatch, p.targetLength, (unsigned)p.strategy);
}
static void result(size_t code) {
    printf("%u\t%llu", (unsigned)ZSTD_getErrorCode(code),
           ZSTD_isError(code) ? 0ULL : (unsigned long long)code);
}
static unsigned field(ZSTD_compressionParameters p, unsigned i) {
    switch (i) { case 0: return p.windowLog; case 1: return p.chainLog; case 2: return p.hashLog;
        case 3: return p.searchLog; case 4: return p.minMatch; case 5: return p.targetLength;
        default: return (unsigned)p.strategy; }
}
static void set_field(ZSTD_compressionParameters* p, unsigned i, unsigned value) {
    switch (i) { case 0: p->windowLog=value; break; case 1: p->chainLog=value; break;
        case 2: p->hashLog=value; break; case 3: p->searchLog=value; break;
        case 4: p->minMatch=value; break; case 5: p->targetLength=value; break;
        default: p->strategy=(ZSTD_strategy)value; break; }
}
/* Source-derived adapter for the private static ZSTD_clampCParams, :1401-1421.
 * Public adjustment rows independently exercise the actual library clamp.
 * The explicit conversion to int deliberately records this compiler's signed32
 * interpretation of unsigned values rather than inventing an unsigned clamp.
 */
static ZSTD_compressionParameters clamp_for_private_adapter(ZSTD_compressionParameters p) {
    unsigned i;
    for (i=0; i<7; ++i) {
        ZSTD_bounds const b=ZSTD_cParam_getBounds(fields[i]);
        int const v=(int)field(p,i);
        if (v<b.lowerBound) set_field(&p,i,(unsigned)b.lowerBound);
        else if (v>b.upperBound) set_field(&p,i,(unsigned)b.upperBound);
    }
    return p;
}
static ZSTD_compressionParameters typed_adjust(ZSTD_compressionParameters p, U64 source, size_t dict) {
    ZSTD_CCtx_params request;
    okay(ZSTD_CCtxParams_init(&request,1));
    /* Level1 has targetLength0 in all four unadjusted tiers. Every other field
     * is nonzero after clamping, so the actual internal override path selects
     * exactly these seven fields, including targetLength0. */
    request.cParams=clamp_for_private_adapter(p);
    return ZSTD_getCParamsFromCCtxParams(&request,source,dict,ZSTD_cpm_noAttachDict);
}
static void presets(void) {
    unsigned tier,level;
    puts("tier\trow\t" CP_HEADER);
    for (tier=0;tier<4;++tier) for (level=0;level<=22;++level) {
        printf("%u\t%u\t",tier,level);cp(ZSTD_defaultCParameters[tier][level]);putchar('\n');
    }
}
static void bounds(void) {
    unsigned i;
    puts("name\tparameter_id\tlower\tupper\terror_code");
    for (i=0;i<12;++i) {
        ZSTD_bounds const b=ZSTD_cParam_getBounds(requests[i]);
        printf("%s\t%d\t%d\t%d\t%u\n",request_names[i],(int)requests[i],b.lowerBound,b.upperBound,(unsigned)ZSTD_getErrorCode(b.error));
    }
    { ZSTD_bounds const b=ZSTD_cParam_getBounds((ZSTD_cParameter)INT_MAX);
      printf("unsupported\t%d\t%d\t%d\t%u\n",INT_MAX,b.lowerBound,b.upperBound,(unsigned)ZSTD_getErrorCode(b.error)); }
}
static void width_bounds(void) {
    puts("width\tclassification\twindow_log_max\tchain_log_max\thash_log_max\tsearch_log_max");
    printf("32\tmacro_derived_not_executed\t%d\t%d\t%d\t%d\n",ZSTD_WINDOWLOG_MAX_32,ZSTD_CHAINLOG_MAX_32,
        ZSTD_WINDOWLOG_MAX_32<30?ZSTD_WINDOWLOG_MAX_32:30,ZSTD_WINDOWLOG_MAX_32-1);
    printf("64\thost_executed_bounds\t%d\t%d\t%d\t%d\n",ZSTD_WINDOWLOG_MAX_64,ZSTD_CHAINLOG_MAX_64,
        ZSTD_WINDOWLOG_MAX_64<30?ZSTD_WINDOWLOG_MAX_64:30,ZSTD_WINDOWLOG_MAX_64-1);
}
static void query(const char* label,int internal,int level,U64 source,size_t dict,const char* domain,const char* kind) {
    ZSTD_compressionParameters p; ZSTD_frameParameters f;
    if (internal) {
        ZSTD_CCtx_params request; okay(ZSTD_CCtxParams_init(&request,level));
        p=ZSTD_getCParamsFromCCtxParams(&request,source,dict,ZSTD_cpm_noAttachDict);
        f=request.fParams;
    } else {
        ZSTD_parameters const all=ZSTD_getParams(level,source,dict);
        p=ZSTD_getCParams(level,source,dict); f=all.fParams;
        require(memcmp(&p,&all.cParams,sizeof(p))==0,"getParams/getCParams mismatch");
    }
    printf("%s\t%s\t%d\t%llu\t%llu\t%s\t%s\t",label,internal?"typed_no_attach":"public",level,
        (unsigned long long)source,(unsigned long long)dict,domain,kind);cp(p);
    printf("\t%d\t%d\t%d\t%u\n",f.contentSizeFlag,f.checksumFlag,!f.noDictIDFlag,(unsigned)ZSTD_getErrorCode(ZSTD_checkCParams(p)));
}
static void queries(void) {
    int const levels[]={INT_MIN,-131073,-131072,-999,-9,-1,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,INT_MAX};
    U64 const tiers[]={16384,131072,262144,262145};
    unsigned i,j,k;int delta;
    puts("case\tapi\tlevel\tsource\tdictionary\tdomain\tsource_kind\t" CP_HEADER "\tcontent_size\tchecksum\tdictionary_id\tvalidation_error");
    for(i=0;i<sizeof(levels)/sizeof(levels[0]);++i)for(j=0;j<4;++j)
        query("level_tiers",0,levels[i],tiers[j],0,"ordinary","known");
    for(i=0;i<3;++i)for(delta=-1;delta<=1;++delta)for(j=0;j<4;++j) {
        int const selected[]={-1,2,7,22};
        query("source_tier_boundary",0,selected[j],tiers[i]+delta,0,"ordinary","known");
    }
    for(i=0;i<3;++i)for(delta=-1;delta<=1;++delta)for(j=0;j<2;++j) {
        int const level=j?22:7;
        for(k=0;k<2;++k) { size_t const dict=k?4096:1;
            query("known_dictionary_tier",0,level,tiers[i]+delta-dict,dict,"ordinary","known"); }
        query("unknown_dictionary_plus499",0,level,0,(size_t)(tiers[i]+delta-499),"ordinary","public_zero_unknown");
    }
    { U64 const sizes[]={0,1,511,512,513,1023,1024,1025,(1ULL<<30)-1,1ULL<<30,(1ULL<<30)+1,
        (1ULL<<31)-1,1ULL<<31,(1ULL<<31)+1,UINT32_MAX,UINT64_MAX-1,UINT64_MAX};
      for(i=0;i<sizeof(sizes)/sizeof(sizes[0]);++i)for(j=0;j<2;++j) {
        query("size_resolution",0,j?22:3,sizes[i],0,"ordinary",sizes[i]==0?"public_zero_unknown":sizes[i]==UNKNOWN?"unknown":"known");
        if(sizes[i]==0 || sizes[i]==UNKNOWN)query("typed_empty_or_unknown",1,j?22:3,sizes[i],0,"ordinary",sizes[i]==0?"known":"unknown");
      }
    }
    { size_t const dictionaries[]={1,1024,16384,131072,262144};
      for(i=0;i<sizeof(dictionaries)/sizeof(dictionaries[0]);++i) {
        query("dictionary_empty_vs_unknown",0,7,0,dictionaries[i],"ordinary","public_zero_unknown");
        query("dictionary_empty_vs_unknown",1,7,0,dictionaries[i],"ordinary","known");
      }
    }
    query("reserved_known_sentinel",1,3,UNKNOWN,0,"rust_reject_reserved_known","reserved_known");
    query("known_sum_wrap",0,7,UINT64_MAX-10,20,"c_modular_known_sum","known");
    query("unknown_dict_plus499_wrap",0,7,UNKNOWN,SIZE_MAX-100,"c_modular_dictionary_plus499","unknown");
    query("dictionary_window_wrap",0,22,10,SIZE_MAX-20,"c_modular_dictionary_window","known");
}
static void adjustment(const char* label,int internal,ZSTD_compressionParameters p,U64 source,size_t dict,const char* domain) {
    ZSTD_compressionParameters const adjusted=internal?typed_adjust(p,source,dict):ZSTD_adjustCParams(p,source,dict);
    printf("%s\t%s\t%llu\t%llu\t%s\t",label,internal?"typed_no_attach_adapter":"public",(unsigned long long)source,(unsigned long long)dict,domain);
    cp(p);printf("\t%u\t",(unsigned)ZSTD_getErrorCode(ZSTD_checkCParams(p)));cp(adjusted);
    printf("\t%u\n",(unsigned)ZSTD_getErrorCode(ZSTD_checkCParams(adjusted)));
}
static void adjustments(void) {
    ZSTD_compressionParameters const base=ZSTD_getCParams(3,UNKNOWN,0);unsigned i,j,s;
    puts("case\tapi\tsource\tdictionary\tdomain\tinput_window_log\tinput_chain_log\tinput_hash_log\tinput_search_log\tinput_min_match\tinput_target_length\tinput_strategy\tinput_validation_error\t" CP_HEADER "\tvalidation_error");
    for(i=0;i<7;++i) {
        ZSTD_bounds const b=ZSTD_cParam_getBounds(fields[i]);
        unsigned const values[]={0,(unsigned)(b.lowerBound-1),(unsigned)b.lowerBound,(unsigned)b.upperBound,(unsigned)(b.upperBound+1),INT_MAX,0x80000000U,UINT_MAX};
        for(j=0;j<sizeof(values)/sizeof(values[0]);++j) { ZSTD_compressionParameters p=base;set_field(&p,i,values[j]);
            adjustment(names[i],0,p,0,0,"ordinary_signed32_raw");
            adjustment(names[i],1,p,0,0,"ordinary_signed32_raw"); }
    }
    for(s=1;s<=9;++s)for(j=0;j<5;++j) {
        unsigned const searches[]={1,4,5,6,ZSTD_SEARCHLOG_MAX};
        ZSTD_compressionParameters p={ZSTD_WINDOWLOG_MAX,ZSTD_CHAINLOG_MAX,ZSTD_HASHLOG_MAX,searches[j],3,0,(ZSTD_strategy)s};
        adjustment("strategy_hash_cycle_limits",0,p,UNKNOWN,0,"ordinary");
        adjustment("small_source_with_dictionary",1,p,1024,1<<20,"ordinary");
    }
    { U64 const sizes[]={1,511,512,513,1023,1024,1025,(1ULL<<30)-1,1ULL<<30,(1ULL<<30)+1};
      for(i=0;i<sizeof(sizes)/sizeof(sizes[0]);++i)for(j=0;j<2;++j)
        adjustment("source_adjustment_boundary",0,ZSTD_defaultCParameters[0][22],sizes[i],j?4096:0,"ordinary"); }
    adjustment("all_unsigned_max",0,(ZSTD_compressionParameters){UINT_MAX,UINT_MAX,UINT_MAX,UINT_MAX,UINT_MAX,UINT_MAX,(ZSTD_strategy)UINT_MAX},UNKNOWN,0,"ordinary_signed32_raw");
    adjustment("known_sum_wrap",0,base,UINT64_MAX-10,20,"c_modular_known_sum");
    adjustment("dictionary_window_wrap",0,base,10,SIZE_MAX-20,"c_modular_dictionary_window");
}
static void requested_values(const ZSTD_CCtx_params* request) {
    unsigned i;int value;
    for(i=0;i<12;++i) {okay(ZSTD_CCtxParams_getParameter(request,requests[i],&value));printf("\t%d",value);}
}
static void request_row(const char* scenario,unsigned step,const char* operation,int parameter,int value,size_t code,const ZSTD_CCtx_params* request) {
    printf("%s\t%u\t%s\t%d\t%d\t",scenario,step,operation,parameter,value);result(code);requested_values(request);
    putchar('\t');cp(ZSTD_getCParamsFromCCtxParams(request,0,0,ZSTD_cpm_noAttachDict));
    putchar('\t');cp(ZSTD_getCParamsFromCCtxParams(request,UNKNOWN,0,ZSTD_cpm_noAttachDict));
    putchar('\t');cp(ZSTD_getCParamsFromCCtxParams(request,16385,0,ZSTD_cpm_noAttachDict));putchar('\n');
}
static void request_vectors(void) {
    unsigned i,j;ZSTD_CCtx_params request;
    puts("scenario\tstep\toperation\tparameter_id\tvalue\terror_code\treturn_value\tlevel\twindow_log\tchain_log\thash_log\tsearch_log\tmin_match\ttarget_length\tstrategy\tsource_hint\tcontent_size\tchecksum\tdictionary_id\tempty_window\tempty_chain\tempty_hash\tempty_search\tempty_match\tempty_target\tempty_strategy\tunknown_window\tunknown_chain\tunknown_hash\tunknown_search\tunknown_match\tunknown_target\tunknown_strategy\tknown_window\tknown_chain\tknown_hash\tknown_search\tknown_match\tknown_target\tknown_strategy");
    for(i=0;i<12;++i) {
        ZSTD_bounds const b=ZSTD_cParam_getBounds(requests[i]);
        int const values[]={INT_MIN,-1,0,b.lowerBound-1,b.lowerBound,b.upperBound,b.upperBound==INT_MAX?INT_MAX:b.upperBound+1,INT_MAX};
        okay(ZSTD_CCtxParams_init(&request,3));request_row(request_names[i],0,"init",0,3,0,&request);
        for(j=0;j<sizeof(values)/sizeof(values[0]);++j) {
            ZSTD_CCtx_params const before=request;
            size_t const code=ZSTD_CCtxParams_setParameter(&request,requests[i],values[j]);
            if(ZSTD_isError(code)) require(memcmp(&before,&request,sizeof(request))==0,"failed parameter setter mutated requested state");
            request_row(request_names[i],j+1,"set",(int)requests[i],values[j],code,&request);
        }
        request_row(request_names[i],9,"reset",0,0,ZSTD_CCtxParams_reset(&request),&request);
    }
    for(i=0;i<2;++i) {
        const char* scenario=i?"level_then_hash":"hash_then_level";
        okay(ZSTD_CCtxParams_init(&request,3));request_row(scenario,0,"init",0,3,0,&request);
        request_row(scenario,1,"set",i?100:102,i?9:14,ZSTD_CCtxParams_setParameter(&request,i?ZSTD_c_compressionLevel:ZSTD_c_hashLog,i?9:14),&request);
        request_row(scenario,2,"set",i?102:100,i?14:9,ZSTD_CCtxParams_setParameter(&request,i?ZSTD_c_hashLog:ZSTD_c_compressionLevel,i?14:9),&request);
        request_row(scenario,3,"set",100,1,ZSTD_CCtxParams_setParameter(&request,ZSTD_c_compressionLevel,1),&request);
        request_row(scenario,4,"set",102,0,ZSTD_CCtxParams_setParameter(&request,ZSTD_c_hashLog,0),&request);
    }
    okay(ZSTD_CCtxParams_init(&request,3));
    request_row("source_hint_small",0,"init",0,3,0,&request);
    request_row("source_hint_small",1,"set",(int)ZSTD_c_srcSizeHint,16384,
        ZSTD_CCtxParams_setParameter(&request,ZSTD_c_srcSizeHint,16384),&request);
    { unsigned const values[]={20,16,18,4,4,25,4};
      okay(ZSTD_CCtxParams_init(&request,3));request_row("all_overrides",0,"init",0,3,0,&request);
      for(i=0;i<7;++i)request_row("all_overrides",i+1,"set",(int)fields[i],(int)values[i],ZSTD_CCtxParams_setParameter(&request,fields[i],(int)values[i]),&request);
      request_row("all_overrides",8,"set",100,22,ZSTD_CCtxParams_setParameter(&request,ZSTD_c_compressionLevel,22),&request);
      request_row("all_overrides",9,"set",100,-9,ZSTD_CCtxParams_setParameter(&request,ZSTD_c_compressionLevel,-9),&request);
      for(i=0;i<7;++i)request_row("all_overrides",i+10,"set",(int)fields[i],0,ZSTD_CCtxParams_setParameter(&request,fields[i],0),&request);
    }
    okay(ZSTD_CCtxParams_init(&request,3));
    request_row("unsupported",0,"set",INT_MAX,1,ZSTD_CCtxParams_setParameter(&request,(ZSTD_cParameter)INT_MAX,1),&request);
}
static unsigned char dictionary_bytes[16384];
static size_t dictionary_size;
static const unsigned char source_bytes[]="abcabcabcabcabcabcabcabcabcabcabc";
static void lifecycle_row(const char* scenario,unsigned step,const char* operation,int argument,
                          size_t code,ZSTD_CCtx* context,size_t consumed,size_t produced,
                          const void* frame,size_t frame_size) {
    unsigned i;ZSTD_frameHeader header;int has_header=0;
    if(frame_size && !ZSTD_isError(ZSTD_getFrameHeader(&header,frame,frame_size)) &&
       ZSTD_getFrameHeader(&header,frame,frame_size)==0)has_header=1;
    printf("%s\t%u\t%s\t%d\t",scenario,step,operation,argument);result(code);
    printf("\t%u\t%d\t%llu\t%llu\t%llu\t%d\t%d\t%llu\t%u",(unsigned)context->streamStage,
        context->cParamsChanged,(unsigned long long)context->pledgedSrcSizePlusOne,
        (unsigned long long)consumed,(unsigned long long)produced,context->cdict!=NULL,
        context->localDict.dict!=NULL,(unsigned long long)context->prefixDict.dictSize,context->dictID);
    for(i=0;i<12;++i){int value;okay(ZSTD_CCtx_getParameter(context,requests[i],&value));printf("\t%d",value);}
    putchar('\t');cp(context->appliedParams.cParams);
    printf("\t%d\t%llu\t%llu\t%d\n",has_header,
        has_header?(unsigned long long)header.dictID:0ULL,
        has_header?(unsigned long long)header.frameContentSize:0ULL,has_header?(int)header.checksumFlag:0);
}
static size_t process_row(const char* scenario,unsigned step,const char* operation,
                          ZSTD_CCtx* context,size_t input_size,size_t capacity,ZSTD_EndDirective end) {
    unsigned char bytes[4096];ZSTD_inBuffer input={source_bytes,input_size,0};
    ZSTD_outBuffer output={bytes,capacity,0};size_t code;
    require(input_size<=sizeof(source_bytes) && capacity<=sizeof(bytes),"bounded lifecycle buffers");
    code=ZSTD_compressStream2(context,&output,&input,end);
    if(!strcmp(operation,"partial_header"))require(output.pos==1 && bytes[0]==0x28,"partial header must emit exactly one magic byte");
    if(!strcmp(operation,"pledge_error"))require(ZSTD_getErrorCode(code)==ZSTD_error_srcSize_wrong,"started-session pledge mismatch must reject");
    else okay(code);
    if(!strcmp(operation,"pending_flush") || !strcmp(operation,"pending_end"))
        require(code>0,"pending operation must need more output");
    if(!strcmp(operation,"ended") || !strcmp(operation,"first_end") ||
       !strcmp(operation,"first_frame") || !strcmp(operation,"next_frame") ||
       !strcmp(operation,"automatic_next_frame"))
        require(code==0 && input.pos==input.size,"completed operation must consume the complete input and finish");
    lifecycle_row(scenario,step,operation,(int)end,code,context,input.pos,output.pos,bytes,output.pos);
    return code;
}
static ZSTD_CCtx* configured(const char* attachment,const ZSTD_CDict* prepared) {
    ZSTD_CCtx* context=ZSTD_createCCtx();require(context!=NULL,"create context");
    okay(ZSTD_CCtx_setParameter(context,ZSTD_c_compressionLevel,6));
    okay(ZSTD_CCtx_setParameter(context,ZSTD_c_windowLog,10));
    okay(ZSTD_CCtx_setParameter(context,ZSTD_c_hashLog,14));
    okay(ZSTD_CCtx_setParameter(context,ZSTD_c_checksumFlag,1));
    if(!strcmp(attachment,"loaded"))okay(ZSTD_CCtx_loadDictionary(context,dictionary_bytes,dictionary_size));
    else if(!strcmp(attachment,"cdict"))okay(ZSTD_CCtx_refCDict(context,prepared));
    else if(!strcmp(attachment,"prefix"))okay(ZSTD_CCtx_refPrefix_advanced(context,dictionary_bytes,dictionary_size,ZSTD_dct_fullDict));
    return context;
}
static void lifecycle(void) {
    const char* phases[]={"fresh","empty_continue","partial_header","active_input","pending_flush","pending_end","ended","pledge_error"};
    const char* attachments[]={"none","loaded","cdict","prefix"};
    ZSTD_CDict* prepared;unsigned i,j;
    ZSTD_compressionParameters const dictionary_params=ZSTD_getCParams(3,128,dictionary_size);
    prepared=ZSTD_createCDict_advanced(dictionary_bytes,dictionary_size,ZSTD_dlm_byRef,ZSTD_dct_fullDict,dictionary_params,ZSTD_defaultCMem);
    require(prepared!=NULL,"create prepared dictionary");
    puts("scenario\tstep\toperation\targument\terror_code\treturn_value\tstream_stage\tparameters_changed\tpledged_plus_one\tconsumed\tproduced\tattached_cdict\tloaded_dictionary\tprefix_size\tstored_dictionary_id\tlevel\twindow_log\tchain_log\thash_log\tsearch_log\tmin_match\ttarget_length\tstrategy\tsource_hint\tcontent_size\tchecksum\tdictionary_id\tapplied_window\tapplied_chain\tapplied_hash\tapplied_search\tapplied_match\tapplied_target\tapplied_strategy\theader_present\theader_dictionary_id\theader_content_size\theader_checksum");
    for(i=0;i<8;++i)for(j=1;j<=3;++j) {
        char scenario[80];ZSTD_CCtx* context=configured("none",prepared);size_t code;
        snprintf(scenario,sizeof(scenario),"phase_%s_reset_%u",phases[i],j);
        okay(ZSTD_CCtx_setPledgedSrcSize(context,(i==5 || i==6)?16:i==7?1:64));
        if(i==0)lifecycle_row(scenario,0,"configured",0,0,context,0,0,NULL,0);
        if(i==1)process_row(scenario,0,"empty_continue",context,0,0,ZSTD_e_continue);
        if(i==2)process_row(scenario,0,"partial_header",context,16,1,ZSTD_e_flush);
        if(i==3)process_row(scenario,0,"active_input",context,16,4096,ZSTD_e_continue);
        if(i==4)process_row(scenario,0,"pending_flush",context,16,0,ZSTD_e_flush);
        if(i==5)process_row(scenario,0,"pending_end",context,16,0,ZSTD_e_end);
        if(i==6)process_row(scenario,0,"ended",context,16,4096,ZSTD_e_end);
        if(i==7) {
            okay(process_row(scenario,0,"pledge_prefix",context,1,0,ZSTD_e_continue));
            process_row(scenario,1,"pledge_error",context,1,4096,ZSTD_e_end);
        }
        code=ZSTD_CCtx_reset(context,(ZSTD_ResetDirective)j);
        lifecycle_row(scenario,i==7?2:1,"reset",(int)j,code,context,0,0,NULL,0);
        if(ZSTD_isError(code)) {
            code=ZSTD_CCtx_reset(context,ZSTD_reset_session_and_parameters);
            lifecycle_row(scenario,i==7?3:2,"recover_reset_all",3,code,context,0,0,NULL,0);
        }
        /* A parameter-only reset retains a fresh context's pledge. Set the next
         * frame's exact pledge explicitly rather than accidentally testing a
         * leftover source-size error while observing reset/header defaults. */
        okay(ZSTD_CCtx_setPledgedSrcSize(context,16));
        okay(process_row(scenario,i==7?4:3,"next_frame",context,16,4096,ZSTD_e_end));
        ZSTD_freeCCtx(context);
    }
    { ZSTD_CCtx* context=configured("none",prepared);
      size_t const code=ZSTD_CCtx_setPledgedSrcSize(context,1);
      lifecycle_row("first_end_overrides_pledge",0,"set_pledge",1,code,context,0,0,NULL,0);okay(code);
      okay(process_row("first_end_overrides_pledge",1,"first_end",context,2,4096,ZSTD_e_end));
      ZSTD_freeCCtx(context);
    }
    for(i=0;i<4;++i)for(j=1;j<=3;++j) {
        char scenario[80];ZSTD_CCtx* context=configured(attachments[i],prepared);size_t code;
        snprintf(scenario,sizeof(scenario),"dictionary_%s_reset_%u",attachments[i],j);
        lifecycle_row(scenario,0,"configured",0,0,context,0,0,NULL,0);
        code=ZSTD_CCtx_reset(context,(ZSTD_ResetDirective)j);
        lifecycle_row(scenario,1,"reset",(int)j,code,context,0,0,NULL,0);okay(code);
        okay(process_row(scenario,2,"first_frame",context,16,4096,ZSTD_e_end));
        /* C automatically accepts another frame after completion. Rust's
         * explicit Finished state intentionally requires a session reset. */
        okay(process_row(scenario,3,"automatic_next_frame",context,16,4096,ZSTD_e_end));
        ZSTD_freeCCtx(context);
    }
    for(i=1;i<4;++i)for(j=1;j<=3;++j) {
        char scenario[80];ZSTD_CCtx* context=configured(attachments[i],prepared);size_t code;
        snprintf(scenario,sizeof(scenario),"completed_%s_reset_%u",attachments[i],j);
        okay(process_row(scenario,0,"first_frame",context,16,4096,ZSTD_e_end));
        code=ZSTD_CCtx_reset(context,(ZSTD_ResetDirective)j);
        lifecycle_row(scenario,1,"reset_after_frame",(int)j,code,context,0,0,NULL,0);okay(code);
        okay(process_row(scenario,2,"next_frame",context,16,4096,ZSTD_e_end));
        ZSTD_freeCCtx(context);
    }
    for(i=0;i<4;++i) {
        char scenario[80];ZSTD_CCtx* context=configured(attachments[i],prepared);ZSTD_CCtx_params request;size_t code;
        snprintf(scenario,sizeof(scenario),"bulk_%s",attachments[i]);okay(ZSTD_CCtxParams_init(&request,9));
        okay(ZSTD_CCtxParams_setParameter(&request,ZSTD_c_hashLog,12));
        lifecycle_row(scenario,0,"configured",0,0,context,0,0,NULL,0);
        code=ZSTD_CCtx_setParametersUsingCCtxParams(context,&request);
        lifecycle_row(scenario,1,"bulk_fresh",0,code,context,0,0,NULL,0);
        process_row(scenario,2,"start",context,16,0,ZSTD_e_continue);
        code=ZSTD_CCtx_setParametersUsingCCtxParams(context,&request);
        lifecycle_row(scenario,3,"bulk_active",0,code,context,0,0,NULL,0);
        code=ZSTD_CCtx_setParameter(context,ZSTD_c_hashLog,INT_MAX);
        lifecycle_row(scenario,4,"invalid_hash_active",INT_MAX,code,context,0,0,NULL,0);
        code=ZSTD_CCtx_setParameter(context,ZSTD_c_hashLog,13);
        lifecycle_row(scenario,5,"valid_hash_active",13,code,context,0,0,NULL,0);
        code=ZSTD_CCtx_setParameter(context,ZSTD_c_windowLog,11);
        lifecycle_row(scenario,6,"window_active",11,code,context,0,0,NULL,0);
        ZSTD_freeCCtx(context);
    }
    ZSTD_freeCDict(prepared);
}
int main(int argc,char** argv) {
    require(ZSTD_versionNumber()==10507,"wrong zstd oracle version");
    require(sizeof(size_t)==8 && sizeof(int)==4 && sizeof(unsigned)==4,"requires captured 64-bit size_t / 32-bit int C host");
    require(sizeof(ZSTD_compressionParameters)==28 && sizeof(ZSTD_strategy)==4 &&
        (int)UINT_MAX==-1 && (int)0x80000000U==INT_MIN,"requires captured seven-field / signed32 conversion ABI");
    require(argc>=2,"expected fixture section");
    if(!strcmp(argv[1],"presets"))presets();
    else if(!strcmp(argv[1],"bounds"))bounds();
    else if(!strcmp(argv[1],"width-bounds"))width_bounds();
    else if(!strcmp(argv[1],"queries"))queries();
    else if(!strcmp(argv[1],"adjustments"))adjustments();
    else if(!strcmp(argv[1],"requests"))request_vectors();
    else if(!strcmp(argv[1],"lifecycle")) {
        FILE* file;require(argc==3,"expected formatted dictionary path");file=fopen(argv[2],"rb");require(file!=NULL,"open dictionary");
        dictionary_size=fread(dictionary_bytes,1,sizeof(dictionary_bytes),file);require(!ferror(file) && feof(file),"bounded dictionary read");
        fclose(file);require(dictionary_size>=8,"dictionary size");lifecycle();
    } else require(0,"unknown fixture section");
    return ferror(stdout)?1:0;
}
