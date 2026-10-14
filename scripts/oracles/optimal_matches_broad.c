/* Bounded observer for unchanged facebook/zstd v1.5.7 sorted-tree functions.
 * Revision f8745da6ff1ad1e7bab384bd1f9d742439278e99; LICENSE.zstd.
 * Includes zstd_opt.c in place of its separate object. No codec implementation.
 */
#define _POSIX_C_SOURCE 200809L
#define ZSTD_STATIC_LINKING_ONLY
#include "compress/zstd_opt.c"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#define SOURCE_CAP 524288u
#define QUERY_CAP 16u
#define PACKET_CAP 524716u
#define CELL_CAP 4096u
#define SHORT_CAP 131072u
#define MATCH_CAP 4099u
#define SENTINEL UINT32_C(0xa59bd763)
#define TABLE(name, count) static struct { U32 before[16], values[count], after[16]; } name
TABLE(heads, CELL_CAP);
TABLE(nodes, CELL_CAP);
TABLE(short_heads, SHORT_CAP);
static struct { unsigned char before[64], values[SOURCE_CAP+2], after[64]; } arena;
static struct { U32 before[16]; ZSTD_match_t values[MATCH_CAP]; U32 after[16]; } candidates;
static U32 saved_heads[CELL_CAP], saved_nodes[CELL_CAP], saved_short[SHORT_CAP];
static U32 queries[QUERY_CAP][6];
static unsigned char packet[PACKET_CAP];
static ZSTD_MatchState_t state;
static char output_buffer[65536];
_Static_assert(sizeof(U32)==4, "32-bit cells");
_Static_assert(sizeof(heads)+sizeof(nodes)+sizeof(short_heads)+sizeof(arena)+sizeof(candidates)+
               sizeof(saved_heads)+sizeof(saved_nodes)+sizeof(saved_short)+sizeof(queries)+
               sizeof(packet)+sizeof(state)+sizeof(output_buffer)+65536u < (4u<<20),
               "bounded observer-controlled storage");

static void need(int ok, const char *why) {
    if (!ok) { fprintf(stderr,"optimal match oracle: %s\n",why); exit(1); }
}
static U32 little32(const unsigned char *p) {
    return (U32)p[0]|((U32)p[1]<<8)|((U32)p[2]<<16)|((U32)p[3]<<24);
}
static void guard(U32 *before,U32 *after,int initialize) {
    U32 i;
    for (i=0;i<16;i++) {
        if (initialize) before[i]=after[i]=SENTINEL;
        else need(before[i]==SENTINEL && after[i]==SENTINEL,"cell guard changed");
    }
}
static void initialize_table(U32 *data,U32 live,U32 capacity) {
    U32 i;
    for (i=0;i<capacity;i++) data[i]=i<live ? 0 : SENTINEL;
}
static void check_table(const U32 *data,U32 live,U32 capacity) {
    U32 i;
    for (i=live;i<capacity;i++) need(data[i]==SENTINEL,"inactive cell changed");
}
static void check_guards(U32 n,U32 h,U32 c,U32 h3,unsigned char padding,U32 count) {
    U32 i;
    guard(heads.before,heads.after,0); guard(nodes.before,nodes.after,0);
    guard(short_heads.before,short_heads.after,0); guard(candidates.before,candidates.after,0);
    check_table(heads.values,h,CELL_CAP);check_table(nodes.values,c,CELL_CAP);check_table(short_heads.values,h3,SHORT_CAP);
    for(i=0;i<64;i++) need(arena.before[i]==padding && arena.after[i]==padding,"arena guards changed");
    need(arena.values[0]==padding && arena.values[1]==padding,"prefix padding changed");
    need(memcmp(arena.values+2,packet+44,n)==0,"source changed");
    for(i=n+2;i<SOURCE_CAP+2;i++) need(arena.values[i]==padding,"source tail changed");
    for(i=count;i<MATCH_CAP;i++) need(candidates.values[i].off==SENTINEL && candidates.values[i].len==SENTINEL,"unused candidate changed");
}
static void array(const U32 *values,U32 count) {
    U32 i;putchar('[');
    for(i=0;i<count;i++) { if(i) putchar(',');printf("%u",values[i]); }
    putchar(']');
}
static void snapshot(const U32 *h,const U32 *c,const U32 *h3,U32 hc,U32 cc,U32 h3c,U32 frontier,U32 short_frontier) {
    U32 i;
    printf("{\"heads\":");array(h,hc);printf(",\"nodes\":[");
    for(i=0;i<cc;i+=2) {if(i) putchar(',');printf("[%u,%u]",c[i],c[i+1]);}
    printf("],\"short_heads\":");array(h3,h3c);
    printf(",\"frontier\":%u,\"short_frontier\":%u}",frontier,short_frontier);
}
int main(int argc,char **argv) {
    struct stat info;FILE *input;size_t size;
    U32 n,count,w,h,c,s,m,target,h3,i,j,pass,hc,cc,h3c;
    need(argc==2,"one packet path required");
    need(stat(argv[1],&info)==0 && S_ISREG(info.st_mode),"regular packet");
    need(info.st_size>=76 && info.st_size<=PACKET_CAP,"packet extent");
    size=(size_t)info.st_size;input=fopen(argv[1],"rb");need(input!=NULL,"packet open");
    need(fread(packet,1,size,input)==size && fgetc(input)==EOF && !ferror(input),"complete packet");
    need(fclose(input)==0,"packet close");need(memcmp(packet,"ZOPMB02\0",8)==0,"packet magic");
    n=little32(packet+8);count=little32(packet+12);w=little32(packet+16);h=little32(packet+20);
    c=little32(packet+24);s=little32(packet+28);m=little32(packet+32);target=little32(packet+36);h3=little32(packet+40);
    need(n>=8 && n<=SOURCE_CAP && count>=1 && count<=QUERY_CAP,"source/query bounds");
    need(size==44u+n+24u*count,"exact packet size");
    need(w>=10 && w<=21 && h>=6 && h<=12 && c>=6 && c<=12 && s>=1 && s<=8,"geometry");
    need(m>=3 && m<=7 && target<=131072,"minimum/target");
    need(m==3 ? h3==MIN(w,17u) : h3==0,"short-key geometry");
    for(i=0;i<count;i++) {
        U32 *q=queries[i];
        for(j=0;j<6;j++) q[j]=little32(packet+44+n+24*i+4*j);
        need(q[0]<=n-8 && (i==0 || q[0]>queries[i-1][0]),"query order/footprint");
        for(j=1;j<=3;j++) need(q[j]<=4096 && q[j]<=q[0]+2,"repeat pointer bounds");
        need(q[4]<=1 && q[5]>=(m==3 ? 3u : 4u) && q[5]<=4096 && q[5]<=n-q[0],"query flags/length");
    }
    hc=1u<<h;cc=1u<<c;h3c=h3 ? 1u<<h3 : 0;
    need(setvbuf(stdout,output_buffer,_IOFBF,sizeof(output_buffer))==0,"stdout buffer");
    printf("{\"record\":\"metadata\",\"protocol\":\"ZOPMB02\",\"revision\":\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"source_bytes\":%u,\"queries\":%u,\"bias\":2,\"parameters\":[%u,%u,%u,%u,%u,%u,%u]}\n",n,count,w,h,c,s,m,target,h3);
    for(pass=0;pass<2;pass++) {
        unsigned char padding=pass ? 0xc3 : 0x6a;
        U32 next3=2;
        memset(&state,0,sizeof(state));memset(&arena,padding,sizeof(arena));memcpy(arena.values+2,packet+44,n);
        initialize_table(heads.values,hc,CELL_CAP);initialize_table(nodes.values,cc,CELL_CAP);initialize_table(short_heads.values,h3c,SHORT_CAP);
        guard(heads.before,heads.after,1);guard(nodes.before,nodes.after,1);
        guard(short_heads.before,short_heads.after,1);guard(candidates.before,candidates.after,1);
        state.hashTable=heads.values;state.chainTable=nodes.values;state.hashTable3=h3 ? short_heads.values : NULL;
        state.hashLog3=h3;state.nextToUpdate=2;
        state.window.base=arena.values;state.window.dictBase=arena.values;
        state.window.lowLimit=2;state.window.dictLimit=2;state.window.nextSrc=arena.values+2+n;
        state.cParams.windowLog=w;state.cParams.hashLog=h;state.cParams.chainLog=c;
        state.cParams.searchLog=s;state.cParams.minMatch=m;state.cParams.targetLength=target;state.cParams.strategy=ZSTD_btopt;
        for(i=0;i<count;i++) {
            const U32 *q=queries[i];U32 before=state.nextToUpdate,before3=next3,found;
            memcpy(saved_heads,heads.values,hc*sizeof(U32));memcpy(saved_nodes,nodes.values,cc*sizeof(U32));
            memcpy(saved_short,short_heads.values,h3c*sizeof(U32));
            for(j=0;j<MATCH_CAP;j++) candidates.values[j].off=candidates.values[j].len=SENTINEL;
            check_guards(n,hc,cc,h3c,padding,0);
            found=ZSTD_btGetAllMatches_internal(candidates.values,&state,&next3,arena.values+2+q[0],arena.values+2+n,q+1,q[4],q[5],ZSTD_noDict,BOUNDED(3,m,6));
            need(found<=MATCH_CAP,"candidate capacity");check_guards(n,hc,cc,h3c,padding,found);
            printf("{\"record\":\"operation\",\"padding_pass\":%u,\"index\":%u,\"query\":",pass,i);array(q,6);
            printf(",\"before\":");snapshot(saved_heads,saved_nodes,saved_short,hc,cc,h3c,before,before3);
            printf(",\"after\":");snapshot(heads.values,nodes.values,short_heads.values,hc,cc,h3c,state.nextToUpdate,next3);
            printf(",\"matches\":[");
            for(j=0;j<found;j++) {if(j) putchar(',');printf("[%u,%u]",candidates.values[j].off,candidates.values[j].len);}
            puts("],\"source_unchanged\":true,\"guards_unchanged\":true}");
        }
    }
    printf("{\"record\":\"complete\",\"operations\":%u,\"passes\":2,\"allocations\":0}\n",count);
    need(fflush(stdout)==0 && !ferror(stdout),"complete stdout");return 0;
}
