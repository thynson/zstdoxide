/* Policy-tagged frequency observation adapter, not an Ultra implementation.
 * Includes unchanged facebook/zstd v1.5.7 zstd_opt.c for its static price API.
 * Revision f8745da6ff1ad1e7bab384bd1f9d742439278e99; LICENSE.zstd.
 * Compile this unit instead of a separate zstd_opt.c object.
 * Contract: docs/reference-ultra-frequency-protocol.md, version 1.
 */
#define _DARWIN_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#define ZSTD_STATIC_LINKING_ONLY
#include "compress/zstd_opt.c"
#include <fcntl.h>
#include <unistd.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#define SOURCE_CAP 131072u
#define OP_CAP 128u
#define PACKET_CAP 134168u
_Static_assert(sizeof(unsigned) == 4 && sizeof(U32) == 4, "32-bit frequency cells");
#define CANARY UINT32_C(0xa59bd763)
#define TABLE(name, count) static struct { U32 before[16]; unsigned values[count]; U32 after[16]; } name
TABLE(literals, 256);
TABLE(lengths, 36);
TABLE(matches, 53);
TABLE(offsets, 32);
static struct { unsigned char before[64], values[SOURCE_CAP], after[64]; } source;
static unsigned char packet[PACKET_CAP];
static U32 operations[OP_CAP][6];
static ZSTD_entropyCTables_t costs;
static optState_t state;
static char output_buffer[65536];
_Static_assert(sizeof(source) + sizeof(packet) + sizeof(operations) + sizeof(costs) +
               sizeof(state) + sizeof(output_buffer) + sizeof(literals) + sizeof(lengths) +
               sizeof(matches) + sizeof(offsets) + 65536u < 4u * 1024u * 1024u,
               "bounded adapter-controlled storage");

static void need(int ok, const char *reason) {
    if (!ok) { fprintf(stderr, "ultra frequency oracle: %s\n", reason); exit(1); }
}
static U32 little32(const unsigned char *p) {
    return (U32)p[0] | ((U32)p[1] << 8) | ((U32)p[2] << 16) | ((U32)p[3] << 24);
}
static U64 fingerprint(const unsigned char *p, size_t length) {
    U64 value = UINT64_C(0xcbf29ce484222325);
    size_t i;
    for (i = 0; i < length; ++i) value = (value ^ p[i]) * UINT64_C(0x100000001b3);
    return value;
}
static void span(U32 offset, U32 length, U32 total) {
    need(offset <= total && length <= total - offset, "source range");
}
static void guard(U32 *before, U32 *after, int initialize) {
    unsigned i;
    for (i = 0; i < 16; ++i) {
        if (initialize) before[i] = after[i] = CANARY;
        else need(before[i] == CANARY && after[i] == CANARY, "frequency guard changed");
    }
}
static void guards(int initialize, U32 n) {
    size_t i;
    guard(literals.before, literals.after, initialize);
    guard(lengths.before, lengths.after, initialize);
    guard(matches.before, matches.after, initialize);
    guard(offsets.before, offsets.after, initialize);
    if (initialize) {
        memset(&source, 0xa5, sizeof(source));
        memcpy(source.values, packet + 24, n);
    } else {
        for (i = 0; i < 64; ++i)
            need(source.before[i] == 0xa5 && source.after[i] == 0xa5, "source guard changed");
        for (i = n; i < SOURCE_CAP; ++i) need(source.values[i] == 0xa5, "source tail changed");
        need(memcmp(source.values, packet + 24, n) == 0, "source changed");
    }
}
static void array(const unsigned *values, size_t count) {
    size_t i;
    putchar('[');
    for (i = 0; i < count; ++i) { if (i) putchar(','); printf("%u", values[i]); }
    putchar(']');
}
static void record(U32 index, U32 tag, U32 mode, U32 policy, const U32 *query) {
    printf("{\"price_policy\":%u,\"record\":\"operation\",\"index\":%u,\"tag\":%u,\"price_type\":\"%s\",\"literal_mode\":%u,\"frequencies\":{\"lit\":",
           policy, index, tag, state.priceType == zop_predef ? "predefined" : "dynamic", mode);
    array(state.litFreq, 256); printf(",\"ll\":"); array(state.litLengthFreq, 36);
    printf(",\"ml\":"); array(state.matchLengthFreq, 53);
    printf(",\"of\":"); array(state.offCodeFreq, 32);
    printf("},\"sums\":{\"lit\":%u,\"ll\":%u,\"ml\":%u,\"of\":%u},\"base_prices\":{\"lit\":%u,\"ll\":%u,\"ml\":%u,\"of\":%u},\"query\":",
           state.litSum, state.litLengthSum, state.matchLengthSum, state.offCodeSum,
           state.litSumBasePrice, state.litLengthSumBasePrice, state.matchLengthSumBasePrice, state.offCodeSumBasePrice);
    if (query) array(query, 3); else printf("null");
    puts("}");
}
int main(int argc, char **argv) {
    struct stat info;
    FILE *input;
    int fd;
    U32 n, count, mode, policy, i, j, updates = 0;
    U64 updated_literals = 0, initial_literal_mass = 0;
    int initialized = 0, dirty = 0, literal_mass = 0, predefined = 0;
    size_t size;
    need(argc == 2, "one regular packet path required");
    fd = open(argv[1], O_RDONLY | O_NOFOLLOW | O_NONBLOCK); need(fd >= 0, "open packet");
    need(fstat(fd, &info) == 0 && S_ISREG(info.st_mode) &&
         info.st_size >= 48 && (uintmax_t)info.st_size <= PACKET_CAP, "regular bounded packet");
    size = (size_t)info.st_size;
    input = fdopen(fd, "rb"); need(input != NULL, "open packet");
    need(fread(packet, 1, size, input) == size, "complete packet");
    need(fgetc(input) == EOF && !ferror(input), "exact EOF");
    need(fclose(input) == 0, "close packet");
    need(memcmp(packet, "ZUFREQ01", 8) == 0, "version 1 magic");
    n = little32(packet + 8); count = little32(packet + 12); mode = little32(packet + 16); policy = little32(packet + 20);
    need(n <= SOURCE_CAP && count >= 1 && count <= OP_CAP && mode <= 1 && (policy == 0 || policy == 2), "header bounds");
    need(size == 24u + (size_t)n + 24u * count, "exact packet extent");
    /* Validate the entire schedule before entering upstream or printing output. */
    for (i = 0; i < count; ++i) {
        U32 *op = operations[i];
        U32 tag, a, b, c, d, e;
        for (j = 0; j < 6; ++j) op[j] = little32(packet + 24u + n + 24u * i + 4u * j);
        tag=op[0]; a=op[1]; b=op[2]; c=op[3]; d=op[4]; e=op[5];
        need(tag <= 5, "known tag");
        if (tag == 0) {
            need(i == 0 && !initialized && c == 0 && d == 0 && e == 0, "initialization schedule");
            span(a,b,n); initialized=1; literal_mass=b > 0 && mode == 0; predefined=b <= 8;
            if (mode == 0) {
                U32 histogram[256] = {0};
                U32 k;
                for (k=0; k<b; ++k) histogram[packet[24u+a+k]]++;
                for (k=0; k<256; ++k) initial_literal_mass += (histogram[k] != 0) + (histogram[k] >> 8);
            }
        } else {
            need(initialized, "initialize first");
            if (tag == 1 || tag == 5) {
                U32 repetitions = tag == 5 ? e : 1;
                span(a,b,n);
                need(b < SOURCE_CAP && c != 0 && d >= 3 && d <= SOURCE_CAP, "update domain");
                need((tag != 1 || e == 0) && repetitions >= 1 && repetitions <= 8192, "update count/reserved");
                need(repetitions <= 8192 - updates, "primitive update budget");
                updates += repetitions; updated_literals += (U64)b * repetitions;
                need(updated_literals <= 4194304, "literal update budget");
                if (b > 0 && mode == 0) literal_mass=1;
                dirty=1;
            } else if (tag == 2) {
                need(a == 0 && b == 0 && c == 0 && d == 0 && e == 0, "refresh reserved"); dirty=0;
            } else if (tag == 3) {
                span(a,b,n); need(c == 0 && d == 0 && e == 0, "rescale reserved"); predefined=0; dirty=0;
            } else {
                span(a,b,n); need(c <= SOURCE_CAP && d != 0 && e >= 3 && e <= SOURCE_CAP, "query domain");
                need(!dirty && (b == 0 || mode == 1 || predefined || literal_mass), "query lifecycle");
            }
        }
    }
    need(policy == 0 || mode == 1 || initial_literal_mass + 2 * updated_literals <= 8388606, "fractional literal mass");
    need(setvbuf(stdout, output_buffer, _IOFBF, sizeof(output_buffer)) == 0, "bounded output buffer");
    guards(1,n);
    state.litFreq=literals.values; state.litLengthFreq=lengths.values;
    state.matchLengthFreq=matches.values; state.offCodeFreq=offsets.values;
    state.symbolCosts=&costs; state.literalCompressionMode=mode ? ZSTD_ps_disable : ZSTD_ps_auto;
    printf("{\"record\":\"request\",\"schema_version\":1,\"revision\":\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"source_bytes\":%u,\"source_fnv1a64\":\"%016" PRIx64 "\",\"operations\":%u,\"literal_mode\":%u,\"price_policy\":%u}\n",
           n, (uint64_t)fingerprint(source.values,n), count, mode, policy);
    for (i = 0; i < count; ++i) {
        U32 *op=operations[i], tag=op[0], values[3];
        if (tag == 0 || tag == 3) ZSTD_rescaleFreqs(&state, source.values+op[1], op[2], (int)policy);
        else if (tag == 1 || tag == 5) {
            U32 repetitions=tag == 5 ? op[5] : 1;
            for (j=0; j<repetitions; ++j) ZSTD_updateStats(&state,op[2],source.values+op[1],op[3],op[4]);
        } else if (tag == 2) ZSTD_setBasePrices(&state,(int)policy);
        else {
            values[0]=ZSTD_rawLiteralsCost(source.values+op[1],op[2],&state,(int)policy);
            values[1]=ZSTD_litLengthPrice(op[3],&state,(int)policy);
            values[2]=ZSTD_getMatchPrice(op[4],op[5],&state,(int)policy);
        }
        guards(0,n); record(i,tag,mode,policy,tag == 4 ? values : NULL);
    }
    printf("{\"price_policy\":%u,\"record\":\"end\",\"operations\":%u,\"source_fnv1a64\":\"%016" PRIx64 "\"}\n",
           policy, count, (uint64_t)fingerprint(source.values,n));
    need(fflush(stdout) == 0 && !ferror(stdout), "complete output");
    return 0;
}
