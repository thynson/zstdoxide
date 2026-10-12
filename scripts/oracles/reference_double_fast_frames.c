/* Bounded broader public-frame interoperability oracle for pinned zstd v1.5.7.
 * API contract reference: lib/zstd.h ZSTD_CCtx_setParameter, compress2,
 * findFrameCompressedSize, getFrameContentSize and decompress.
 * No upstream implementation code or private context layout is copied here.
 * This helper is test-only and is never linked into the Rust implementation.
 */
#define ZSTD_STATIC_LINKING_ONLY
#include "zstd.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>

#define MAX_SOURCE ((size_t)4194304)
#define MAX_FRAME ((size_t)5242880)
#define GUARD ((size_t)64)
#define REQUEST_HEADER ((size_t)56)

static void fail(const char *message) {
    fprintf(stderr, "reference DoubleFast C oracle: %s\n", message);
    exit(2);
}
static void require(int condition, const char *message) { if (!condition) fail(message); }
static uint32_t word(const unsigned char *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}
static void write_word(uint32_t value) {
    unsigned char bytes[4]; size_t i;
    for (i=0; i<4; ++i) bytes[i]=(unsigned char)(value >> (8*i));
    require(fwrite(bytes,1,4,stdout)==4, "response word write");
}
static void read_exact(unsigned char *p, size_t size) {
    require(fread(p,1,size,stdin)==size, "truncated request");
}
typedef struct { unsigned char *base, *data; size_t capacity; } Buffer;
static Buffer buffer(size_t capacity) {
    Buffer b;
    require(capacity<=MAX_FRAME && capacity<=SIZE_MAX-2*GUARD, "allocation extent");
    b.base=(unsigned char*)malloc(capacity+2*GUARD);
    require(b.base!=NULL, "allocation failed");
    b.data=b.base+GUARD; b.capacity=capacity;
    memset(b.base,0xa5,capacity+2*GUARD);
    return b;
}
static void guards(Buffer b) {
    size_t i;
    for (i=0;i<GUARD;++i)
        require(b.base[i]==0xa5 && b.data[b.capacity+i]==0xa5, "changed guard");
}
static void checked(size_t code, const char *operation) {
    if (ZSTD_isError(code)) {
        fprintf(stderr,"reference DoubleFast C oracle: %s: %s\n",operation,ZSTD_getErrorName(code));
        exit(2);
    }
}
/* Called only with initialized bytes and an extent within one guarded owner.
 * Header checks precede decompression and prohibit oversized history metadata. */
static void verify_frame(const unsigned char *data, size_t size, size_t source_size) {
    ZSTD_frameHeader header;
    size_t code = ZSTD_getFrameHeader(&header, data, size);
    checked(code, "frame header");
    require(code == 0, "complete frame header");
    require(header.frameType == ZSTD_frame && header.windowSize <= ((size_t)1 << 21)
            && header.frameContentSize == source_size && header.dictID == 0
            && header.checksumFlag == 0, "frame metadata");
    code = ZSTD_findFrameCompressedSize(data, size); checked(code, "frame extent");
    require(code == size, "exact single frame extent");
}
int main(void) {
    unsigned char header[REQUEST_HEADER]; uint32_t f[12];
    size_t i, source_size, frame_size, output_size=0, capacity;
    int level; Buffer source, frame, output; ZSTD_CCtx *context=NULL;
    static const ZSTD_cParameter parameters[7]={ ZSTD_c_windowLog,ZSTD_c_chainLog,
        ZSTD_c_hashLog,ZSTD_c_searchLog,ZSTD_c_minMatch,ZSTD_c_targetLength,ZSTD_c_strategy };
    require(INT_MAX==2147483647, "32-bit int required");
    read_exact(header,REQUEST_HEADER);
    require(memcmp(header,"ZDOI0001",8)==0,"request magic");
    for(i=0;i<12;++i)f[i]=word(header+8+4*i);
    source_size=f[1]; frame_size=f[2];
    require(f[0]<=1 && source_size<=MAX_SOURCE && frame_size<=MAX_FRAME,"mode/extent bounds");
    require((f[0]==0 && frame_size==0)||(f[0]==1 && frame_size>0),"mode/payload agreement");
    /* Decode the signed wire value without implementation-defined unsigned casts. */
    if(f[3]<=INT_MAX) level=(int)f[3];
    else { require(INT_MAX==2147483647,"32-bit int required"); level=-1-(int)(UINT32_MAX-f[3]); }
    for(i=4;i<12;++i)require(f[i]<=INT_MAX,"parameter integer extent");
    {
        static const uint32_t lower[7]={10,6,6,1,3,0,1};
        static const uint32_t upper[7]={31,30,30,30,7,131072,9};
        for(i=0;i<7;++i)
            require(f[4+i]==0 || (lower[i]<=f[4+i] && f[4+i]<=upper[i]), "override domain");
    }
    source=buffer(source_size ? source_size : 1);
    frame=buffer(frame_size ? frame_size : 1);
    read_exact(source.data,source_size); read_exact(frame.data,frame_size);
    require(fgetc(stdin)==EOF && !ferror(stdin),"request exact EOF");
    if(f[0]==0) {
        capacity=ZSTD_compressBound(source_size); checked(capacity,"compress bound");
        require(capacity>0 && capacity<=MAX_FRAME,"compressed output bound");
        output=buffer(capacity);
        context=ZSTD_createCCtx(); require(context!=NULL,"CCtx allocation");
        checked(ZSTD_CCtx_setParameter(context,ZSTD_c_compressionLevel,level),"level");
        for(i=0;i<7;++i)checked(ZSTD_CCtx_setParameter(context,parameters[i],(int)f[i+4]),"override");
        checked(ZSTD_CCtx_setParameter(context,ZSTD_c_srcSizeHint,(int)f[11]),"source hint");
        checked(ZSTD_CCtx_setParameter(context,ZSTD_c_contentSizeFlag,1),"content size flag");
        checked(ZSTD_CCtx_setParameter(context,ZSTD_c_checksumFlag,0),"checksum flag");
        checked(ZSTD_CCtx_setParameter(context,ZSTD_c_dictIDFlag,1),"dictionary flag");
        checked(ZSTD_CCtx_setPledgedSrcSize(context,(unsigned long long)source_size),"known source size");
        output_size=ZSTD_compress2(context,output.data,capacity,source.data,source_size);
        checked(output_size,"compress2"); require(output_size>0 && output_size<=capacity,"compressed size");
        checked(ZSTD_freeCCtx(context),"free CCtx"); context=NULL;
        verify_frame(output.data,output_size,source_size);
    } else {
        verify_frame(frame.data,frame_size,source_size);
        output=buffer(source_size ? source_size : 1);
        output_size=ZSTD_decompress(output.data,source_size,frame.data,frame_size);
        checked(output_size,"decompress");
        require(output_size==source_size && memcmp(output.data,source.data,source_size)==0,"decoded source equality");
    }
    guards(source); guards(frame); guards(output);
    require(fwrite("ZDOC0001",1,8,stdout)==8,"response magic write");
    write_word(0); for(i=0;i<7;++i)write_word(0); /* No claimed applied-parameter view. */
    require(output_size<=UINT32_MAX,"response extent"); write_word((uint32_t)output_size);
    require(fwrite(output.data,1,output_size,stdout)==output_size,"response payload write");
    require(fflush(stdout)==0,"response flush");
    free(output.base);free(frame.base);free(source.base);
    return 0;
}
