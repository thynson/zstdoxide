/* Synthetic scalar observer, not an Ultra implementation.
 * Includes unchanged facebook/zstd v1.5.7 zstd_opt.c; LICENSE.zstd.
 * Revision f8745da6ff1ad1e7bab384bd1f9d742439278e99.
 * Compile instead of a separate zstd_opt.c object. Protocol:
 * docs/reference-ultra-scalar-protocol.md. */
#define _DARWIN_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#define ZSTD_STATIC_LINKING_ONLY
#include "compress/zstd_opt.c"
#include <fcntl.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define ROW_CAP 4096u
#define SOURCE_CAP 131072u
#define PACKET_CAP (12u + ROW_CAP * 40u)
#define GUARD UINT32_C(0xa59bd763)
#define TABLE(name, count) static struct { U32 before[16]; unsigned values[count]; U32 after[16]; } name
TABLE(literals,256); TABLE(lengths,36); TABLE(matches,53); TABLE(offsets,32);
static struct { unsigned char before[64], values[SOURCE_CAP], after[64]; } source;
static unsigned char packet[PACKET_CAP];
static U32 rows[ROW_CAP][10];
static unsigned saved_counts[377];
static optState_t state, saved_state;
static ZSTD_entropyCTables_t costs, saved_costs;
static char output_buffer[65536];
_Static_assert(sizeof(unsigned)==4 && sizeof(U32)==4,"32-bit frequencies");
_Static_assert(sizeof(source)+sizeof(packet)+sizeof(rows)+sizeof(saved_counts)+
    sizeof(state)+sizeof(saved_state)+sizeof(costs)+sizeof(saved_costs)+
    sizeof(literals)+sizeof(lengths)+sizeof(matches)+sizeof(offsets)+
    sizeof(output_buffer)+65536u < 2u*1024u*1024u,"bounded adapter storage");

static void need(int condition) {
    if (!condition) { fputs("ultra scalar oracle: invalid packet\n",stderr); exit(1); }
}
static void unchanged(int condition) {
    if (!condition) { fputs("ultra scalar oracle: mutated storage\n",stderr); exit(2); }
}
static U32 little32(const unsigned char *p) {
    return (U32)p[0]|((U32)p[1]<<8)|((U32)p[2]<<16)|((U32)p[3]<<24);
}
/* Adapter-only format interval lookup; no upstream call during validation. */
static U32 ll_code(U32 length) {
    static const unsigned widths[36]={0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,1,1,1,2,2,3,3,4,6,7,8,9,10,11,12,13,14,15,16};
    U32 base=0,code;
    if (length==SOURCE_CAP) length--;
    for (code=0;code<35;code++) {
        U32 next=base+(1u<<widths[code]);
        if (length<next) return code;
        base=next;
    }
    return 35;
}
static void validate(const U32 *r) {
    U32 mode=r[0],tag=r[1],a=r[2],b=r[3],c=r[4],d=r[5],e=r[6],f=r[7],g=r[8],h=r[9];
    U32 bound;
    need(mode==0 || mode==2); need(tag<=4);
    bound=mode==0 ? UINT32_C(0xfffffffe) : 8388606u;
    if (tag==0) {
        need(a<=bound && !(b|c|d|e|f|g|h));
    } else if (tag==1) {
        need(a<=SOURCE_CAP && d<=1 && !(e|f|g|h));
        need(b<=bound && c<=b && (!d || !(b|c)));
    } else if (tag==2) {
        need(a!=0 && b>=3 && b<=SOURCE_CAP && g<=1 && h==0);
        need(c<=bound && d<=c && e<=bound && f<=e);
        need(!g || !(c|d|e|f));
    } else if (tag==3) {
        need(a<=SOURCE_CAP && e<=1 && f<=1 && !(g|h));
        need(b<=bound && c<=b && d<=b-c);
        need(!(e|f) || !(b|c|d));
        need(a==0 || e || f || mode==2 || b>=1);
    } else {
        need(a>=1 && a<=SOURCE_CAP && e<=1 && !(f|g|h));
        need(b<=bound && c<=b && d<=b && (!e || !(b|c|d)));
        if (ll_code(a-1)==ll_code(a)) need(c==d);
        else need(d<=b-c);
    }
}
static void guard(U32 *before,U32 *after,int initialize) {
    unsigned i;
    for (i=0;i<16;i++) {
        if (initialize) before[i]=after[i]=GUARD;
        else unchanged(before[i]==GUARD && after[i]==GUARD);
    }
}
static void guards(int initialize) {
    size_t i;
    guard(literals.before,literals.after,initialize); guard(lengths.before,lengths.after,initialize);
    guard(matches.before,matches.after,initialize); guard(offsets.before,offsets.after,initialize);
    if (initialize) {
        memset(source.before,0xa5,sizeof(source.before)); memset(source.after,0xa5,sizeof(source.after));
        for (i=0;i<SOURCE_CAP;i++) source.values[i]=(unsigned char)(i%2);
    } else {
        for (i=0;i<64;i++) unchanged(source.before[i]==0xa5 && source.after[i]==0xa5);
        for (i=0;i<SOURCE_CAP;i++) unchanged(source.values[i]==i%2);
    }
}
static void distribution(unsigned *table,U32 total,U32 first,U32 first_count,U32 second,U32 second_count) {
    U32 spare=0;
    table[first]=first_count;
    if (first!=second) table[second]=second_count;
    else second_count=0;
    while (spare==first || spare==second) spare++;
    /* Every table has at least32cells; at most two selected indices excluded. */
    table[spare]=total-first_count-second_count;
}
static void counts(int save) {
    if (save) {
        memcpy(saved_counts,literals.values,256*sizeof(unsigned));
        memcpy(saved_counts+256,lengths.values,36*sizeof(unsigned));
        memcpy(saved_counts+292,matches.values,53*sizeof(unsigned));
        memcpy(saved_counts+345,offsets.values,32*sizeof(unsigned));
    } else {
        unchanged(memcmp(saved_counts,literals.values,256*sizeof(unsigned))==0);
        unchanged(memcmp(saved_counts+256,lengths.values,36*sizeof(unsigned))==0);
        unchanged(memcmp(saved_counts+292,matches.values,53*sizeof(unsigned))==0);
        unchanged(memcmp(saved_counts+345,offsets.values,32*sizeof(unsigned))==0);
    }
}
static int64_t observe(const U32 *r) {
    U32 mode=r[0],tag=r[1],a=r[2],b=r[3],c=r[4],d=r[5],e=r[6],f=r[7],g=r[8];
    int64_t value;
    memset(&state,0,sizeof(state)); memset(&costs,0,sizeof(costs));
    memset(literals.values,0,sizeof(literals.values)); memset(lengths.values,0,sizeof(lengths.values));
    memset(matches.values,0,sizeof(matches.values)); memset(offsets.values,0,sizeof(offsets.values));
    state.litFreq=literals.values; state.litLengthFreq=lengths.values;
    state.matchLengthFreq=matches.values; state.offCodeFreq=offsets.values; state.symbolCosts=&costs;
    state.priceType=zop_dynamic; state.literalCompressionMode=ZSTD_ps_auto;
    if (tag==1) {
        U32 code=ll_code(a);
        state.priceType=d ? zop_predef : zop_dynamic; state.litLengthSum=b;
        distribution(lengths.values,b,code,c,code,c);
    } else if (tag==2) {
        U32 off=ZSTD_highbit32(a),ml=ZSTD_MLcode(b-3);
        state.priceType=g ? zop_predef : zop_dynamic; state.offCodeSum=c; state.matchLengthSum=e;
        distribution(offsets.values,c,off,d,off,d); distribution(matches.values,e,ml,f,ml,f);
    } else if (tag==3) {
        state.priceType=f ? zop_predef : zop_dynamic;
        state.literalCompressionMode=e ? ZSTD_ps_disable : ZSTD_ps_auto; state.litSum=b;
        distribution(literals.values,b,0,c,1,d);
    } else if (tag==4) {
        state.priceType=e ? zop_predef : zop_dynamic; state.litLengthSum=b;
        distribution(lengths.values,b,ll_code(a-1),c,ll_code(a),d);
    }
    counts(1); memcpy(&saved_costs,&costs,sizeof(costs));
    ZSTD_setBasePrices(&state,(int)mode);
    memcpy(&saved_state,&state,sizeof(state));
    if (tag==0) value=mode ? ZSTD_fracWeight(a) : ZSTD_bitWeight(a);
    else if (tag==1) value=ZSTD_litLengthPrice(a,&state,(int)mode);
    else if (tag==2) value=ZSTD_getMatchPrice(a,b,&state,(int)mode);
    else if (tag==3) value=ZSTD_rawLiteralsCost(source.values,a,&state,(int)mode);
    else value=(int64_t)ZSTD_litLengthPrice(a,&state,(int)mode)-(int64_t)ZSTD_litLengthPrice(a-1,&state,(int)mode);
    counts(0); guards(0);
    unchanged(memcmp(&state,&saved_state,sizeof(state))==0 && memcmp(&costs,&saved_costs,sizeof(costs))==0);
    return value;
}
int main(int argc,char **argv) {
    struct stat info;
    int fd;
    FILE *input;
    size_t size;
    U32 count,i,j;
    need(argc==2);
    fd=open(argv[1],O_RDONLY|O_NOFOLLOW|O_NONBLOCK); need(fd>=0);
    need(fstat(fd,&info)==0 && S_ISREG(info.st_mode) && info.st_size>=12 && (uintmax_t)info.st_size<=PACKET_CAP);
    size=(size_t)info.st_size; input=fdopen(fd,"rb"); need(input!=NULL);
    need(fread(packet,1,size,input)==size && fgetc(input)==EOF && !ferror(input)); need(fclose(input)==0);
    need(memcmp(packet,"ZUSCAL01",8)==0); count=little32(packet+8);
    need(count>=1 && count<=ROW_CAP && size==12u+(size_t)count*40u);
    for (i=0;i<count;i++) {
        for (j=0;j<10;j++) rows[i][j]=little32(packet+12u+40u*i+4u*j);
        validate(rows[i]);
    }
    need(setvbuf(stdout,output_buffer,_IOFBF,sizeof(output_buffer))==0); guards(1);
    printf("{\"record\":\"request\",\"revision\":\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"schema_version\":1,\"rows\":%u}\n",count);
    for (i=0;i<count;i++) {
        int64_t value=observe(rows[i]);
        printf("{\"record\":\"scalar\",\"index\":%u,\"mode\":%u,\"tag\":%u,\"value\":%" PRId64 "}\n",i,rows[i][0],rows[i][1],value);
    }
    printf("{\"record\":\"end\",\"rows\":%u,\"guards_unchanged\":true}\n",count);
    unchanged(fflush(stdout)==0 && !ferror(stdout)); return 0;
}
