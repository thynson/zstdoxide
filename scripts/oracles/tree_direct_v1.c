/* Bounded observation adapter, not a tree implementation.
 * Includes unchanged facebook/zstd v1.5.7 zstd_lazy.c to reach its static search
 * entry. Revision f8745da6ff1ad1e7bab384bd1f9d742439278e99; LICENSE.zstd.
 * Compile this translation unit instead of a separate zstd_lazy.c object.
 * Protocol and ownership: docs/reference-tree-oracle.md.
 */
#define _POSIX_C_SOURCE 200809L
#define ZSTD_STATIC_LINKING_ONLY
#include "compress/zstd_lazy.c"
#include "zstd.h"
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#define MAX_SOURCE 131072u
#define MAX_OPERATIONS 256u
#define MAX_PACKET 133200u
#define LIVE_CAP (4u * 1024u * 1024u)
#define GUARD 64u
#define LEADING 64u
#define SUFFIX 2048u
#define OFFSET_SEED 999999999u
#define OWNER_COUNT 5u

typedef struct {
    unsigned char *allocation, *data;
    size_t bytes, cost, leading;
} Owner;
static Owner owners[OWNER_COUNT];
static unsigned char original[MAX_SOURCE];
static char output_buffer[256u * 1024u];
static U32 queries[MAX_OPERATIONS];
static size_t live_bytes, peak_bytes, allocations, releases;
/* Include a conservative 64-KiB allowance for adapter stack/scalar records.
 * The cap describes adapter-controlled storage, not libc/process residency. */
static const size_t fixed_bytes = sizeof(original) + sizeof(output_buffer) +
                                 sizeof(queries) + sizeof(owners) + sizeof(ZSTD_MatchState_t) + 65536u;

static void cleanup(void) {
    unsigned i;
    for (i = 0; i < OWNER_COUNT; ++i) {
        if (owners[i].allocation != NULL) {
            free(owners[i].allocation);
            owners[i].allocation = NULL;
            owners[i].data = NULL;
            ++releases;
        }
    }
    live_bytes = 0;
}
static void require(int ok, const char *reason) {
    if (!ok) {
        fprintf(stderr, "tree oracle: %s\n", reason);
        exit(1); /* Registered cleanup frees exact retained allocation bases. */
    }
}
static size_t add(size_t a, size_t b) {
    require(b <= SIZE_MAX - a, "size addition overflow");
    return a + b;
}
static size_t multiply(size_t a, size_t b) {
    require(a == 0 || b <= SIZE_MAX / a, "size multiplication overflow");
    return a * b;
}
static size_t cost(size_t bytes) { return add(bytes, 2u * GUARD + 63u); }
static void create(unsigned index, size_t bytes) {
    Owner *o;
    size_t adjustment;
    uintptr_t address;
    require(index < OWNER_COUNT && bytes != 0, "owner identity and nonempty layout");
    o = &owners[index];
    require(o->allocation == NULL, "fresh owner");
    o->cost = cost(bytes);
    require(add(fixed_bytes, add(live_bytes, o->cost)) <= LIVE_CAP, "aggregate live cap");
    o->allocation = (unsigned char *)malloc(o->cost);
    require(o->allocation != NULL, "allocation failed");
    ++allocations;
    address = (uintptr_t)(o->allocation + GUARD);
    adjustment = (64u - (address & 63u)) & 63u;
    o->leading = GUARD + adjustment;
    require(o->leading <= o->cost && bytes <= o->cost - o->leading &&
            GUARD <= o->cost - o->leading - bytes, "aligned initialized extent");
    o->data = o->allocation + o->leading;
    o->bytes = bytes;
    require(((uintptr_t)o->data & 63u) == 0, "natural and 64-byte alignment");
    memset(o->allocation, 0xd7, o->leading);
    memset(o->data, 0, bytes);
    memset(o->data + bytes, 0x9b, o->cost - o->leading - bytes);
    live_bytes += o->cost;
    if (live_bytes > peak_bytes) peak_bytes = live_bytes;
}
static void check_owner(const Owner *o) {
    size_t i;
    require(o->allocation != NULL && o->data != NULL, "live initialized owner");
    for (i = 0; i < o->leading; ++i)
        require(o->allocation[i] == 0xd7, "leading guard changed");
    for (i = o->leading + o->bytes; i < o->cost; ++i)
        require(o->allocation[i] == 0x9b, "trailing guard changed");
}
static U32 little32(const unsigned char *p) {
    return (U32)p[0] | ((U32)p[1] << 8) | ((U32)p[2] << 16) | ((U32)p[3] << 24);
}
static U64 fnv(const unsigned char *p, size_t length) {
    U64 value = UINT64_C(0xcbf29ce484222325);
    size_t i;
    for (i = 0; i < length; ++i) {
        value ^= p[i];
        value *= UINT64_C(0x100000001b3);
    }
    return value;
}
static void print_map(const U32 *values, size_t count) {
    size_t i;
    putchar('[');
    for (i = 0; i < count; ++i) {
        if (i != 0) putchar(',');
        printf("%" PRIu32, values[i]);
    }
    putchar(']');
}
static void print_nodes(const U32 *values, size_t cells) {
    size_t i;
    putchar('[');
    for (i = 0; i < cells; i += 2) {
        if (i != 0) putchar(',');
        printf("[%" PRIu32 ",%" PRIu32 "]", values[i], values[i + 1]);
    }
    putchar(']');
}
static void check_source(const unsigned char *source, U32 n, unsigned padding) {
    size_t i;
    require(memcmp(source, original, n) == 0, "logical source changed");
    for (i = 0; i < LEADING; ++i)
        require(owners[0].data[i] == padding, "source leading padding changed");
    for (i = 0; i < SUFFIX; ++i)
        require(source[n + i] == padding, "source suffix changed");
    for (i = 0; i < OWNER_COUNT; ++i) check_owner(&owners[i]);
}
static void check_maps(const U32 *heads, size_t head_count,
                       const U32 *nodes, size_t cells, U32 current) {
    size_t i;
    for (i = 0; i < head_count; ++i)
        require(heads[i] == 0 || (heads[i] >= 2 && heads[i] <= current), "head domain");
    for (i = 0; i < cells; ++i)
        require(nodes[i] == 0 || ((i & 1u) != 0 && nodes[i] == 1) ||
                (nodes[i] >= 2 && nodes[i] <= current), "node cell domain");
}
int main(int argc, char **argv) {
    unsigned char header[80], operation[8];
    struct stat info;
    FILE *input;
    U32 words[18], n, count, i, pass;
    size_t head_count, node_cells, head_bytes, node_bytes, source_bytes, total;
    ZSTD_compressionParameters parameters;
    require(atexit(cleanup) == 0, "register exact-owner cleanup");
    require(setvbuf(stdout, output_buffer, _IOFBF, sizeof(output_buffer)) == 0, "bounded output buffer");
    require(argc == 2, "one regular packet path required");
    require(lstat(argv[1], &info) == 0 && S_ISREG(info.st_mode) &&
            info.st_size >= 96 && (uintmax_t)info.st_size <= MAX_PACKET, "regular bounded packet");
    input = fopen(argv[1], "rb");
    require(input != NULL, "open packet");
    require(fread(header, 1, sizeof(header), input) == sizeof(header), "complete header");
    require(memcmp(header, "ZTREE01\0", 8) == 0, "packet magic");
    for (i = 0; i < 18; ++i) words[i] = little32(header + 8 + 4 * i);
    require(words[0] == 1 && words[1] == 80, "version and header extent");
    n = words[2]; count = words[3];
    require(n >= 8 && n <= MAX_SOURCE && count >= 1 && count <= MAX_OPERATIONS, "source/schedule bounds");
    require(words[4] >= 10 && words[4] <= 17 && words[5] >= 6 && words[5] <= 12 &&
            words[6] >= 6 && words[6] <= 12 && words[7] >= 1 && words[7] <= 8 &&
            words[8] >= 3 && words[8] <= 7, "parameter bounds");
    require(words[9] == 0 && words[10] == 6, "target and strategy");
    for (i = 11; i < 18; ++i) require(words[i] == 0, "initial/reserved fields");
    require((uintmax_t)info.st_size == add(add(80, multiply(8, count)), n), "exact packet extent");
    for (i = 0; i < count; ++i) {
        require(fread(operation, 1, 8, input) == 8, "complete operation");
        require(little32(operation) == 1, "find-only operation");
        queries[i] = little32(operation + 4);
        require(queries[i] <= n - 8 && (i == 0 || queries[i] > queries[i - 1]), "increasing inclusive query");
    }
    require(fread(original, 1, n, input) == n, "complete source");
    require(fgetc(input) == EOF && !ferror(input), "exact packet EOF");
    require(fclose(input) == 0, "close packet");
    memset(&parameters, 0, sizeof(parameters));
    parameters.windowLog = words[4]; parameters.hashLog = words[5];
    parameters.chainLog = words[6]; parameters.searchLog = words[7];
    parameters.minMatch = words[8]; parameters.targetLength = 0;
    parameters.strategy = ZSTD_btlazy2;
    require(!ZSTD_isError(ZSTD_checkCParams(parameters)), "upstream parameter validation");
    head_count = (size_t)1 << parameters.hashLog;
    node_cells = (size_t)1 << parameters.chainLog;
    head_bytes = multiply(head_count, sizeof(U32));
    node_bytes = multiply(node_cells, sizeof(U32));
    source_bytes = add(add(LEADING, n), SUFFIX);
    total = add(fixed_bytes, add(cost(source_bytes), add(multiply(2, cost(head_bytes)), multiply(2, cost(node_bytes)))));
    require(total <= LIVE_CAP, "all layouts preflight before allocation");
    printf("{\"record\":\"metadata\",\"protocol\":\"ZTREE01\",\"revision\":\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"source_bytes\":%" PRIu32 ",\"source_fnv1a64\":\"%016" PRIx64 "\",\"parameters\":[%" PRIu32 ",%" PRIu32 ",%" PRIu32 ",%" PRIu32 ",%" PRIu32 ",0,6],\"queries\":", n, (uint64_t)fnv(original,n), words[4],words[6],words[5],words[7],words[8]);
    print_map(queries,count);
    printf(",\"bias\":2,\"offset_seed\":%u,\"head_cells\":%zu,\"node_cells\":%zu,\"source_arena_bytes\":%zu,\"fixed_bytes\":%zu,\"live_bytes_cap\":%u}\n", OFFSET_SEED,head_count,node_cells,source_bytes,fixed_bytes,LIVE_CAP);
    for (pass = 0; pass < 2; ++pass) {
        ZSTD_MatchState_t state;
        unsigned padding = pass == 0 ? 0x6a : 0xc3;
        unsigned char *source;
        U32 *heads, *nodes, *before_heads, *before_nodes;
        create(0, source_bytes); create(1, head_bytes); create(2, node_bytes);
        create(3, head_bytes); create(4, node_bytes);
        memset(owners[0].data, padding, source_bytes);
        source = owners[0].data + LEADING;
        memcpy(source, original, n);
        heads = (U32 *)(void *)owners[1].data; nodes = (U32 *)(void *)owners[2].data;
        before_heads = (U32 *)(void *)owners[3].data; before_nodes = (U32 *)(void *)owners[4].data;
        memset(&state, 0, sizeof(state));
        state.cParams = parameters;
        state.window.base = source - 2; state.window.dictBase = source - 2;
        state.window.dictLimit = 2; state.window.lowLimit = 2;
        state.window.nextSrc = source + n; state.nextToUpdate = 2;
        state.hashTable = heads; state.chainTable = nodes;
        for (i = 0; i < count; ++i) {
            U32 current = queries[i] + 2;
            U32 before = state.nextToUpdate;
            U32 mask = ((U32)1 << (parameters.chainLog - 1)) - 1;
            U32 tree_low = current > mask ? current - mask : 0;
            U32 window_low = ZSTD_getLowestMatchIndex(&state,current,parameters.windowLog);
            size_t offset = OFFSET_SEED, length;
            memcpy(before_heads, heads, head_bytes); memcpy(before_nodes,nodes,node_bytes);
            length = ZSTD_BtFindBestMatch(&state,source+queries[i],source+n,&offset,
                                        BOUNDED(4,parameters.minMatch,6),ZSTD_noDict);
            check_source(source,n,padding);
            check_maps(heads,head_count,nodes,node_cells,current);
            require(state.nextToUpdate >= 2 && state.nextToUpdate <= n + 2, "frontier source bound");
            require(length <= n - queries[i], "match extent");
            if (length == 0) require(offset == OFFSET_SEED, "zero result seed");
            else {
                size_t distance;
                require(offset > 3, "explicit offset base");
                distance = offset - 3;
                require(distance <= queries[i] && current - distance > window_low, "selected source and window bounds");
                require(memcmp(source+queries[i],source+queries[i]-distance,length)==0,"selected source replay");
            }
            if (current < before)
                require(length == 0 && state.nextToUpdate == before &&
                        memcmp(heads,before_heads,head_bytes)==0 && memcmp(nodes,before_nodes,node_bytes)==0,
                        "skipped query leaves maps/frontier unchanged");
            printf("{\"record\":\"operation\",\"pass\":%" PRIu32 ",\"padding\":%u,\"ordinal\":%" PRIu32 ",\"query\":%" PRIu32 ",\"before_frontier\":%" PRIu32 ",\"after_frontier\":%" PRIu32 ",\"window_floor_raw\":%" PRIu32 ",\"tree_floor_raw\":%" PRIu32 ",\"length\":%zu,\"offset_base\":%zu,\"before_heads\":",pass,padding,i,queries[i],before-2,state.nextToUpdate-2,window_low,tree_low,length,offset);
            print_map(before_heads,head_count); printf(",\"after_heads\":"); print_map(heads,head_count);
            printf(",\"before_nodes\":"); print_nodes(before_nodes,node_cells);
            printf(",\"after_nodes\":"); print_nodes(nodes,node_cells);
            printf(",\"source_unchanged\":true,\"guards_unchanged\":true}\n");
            require(!ferror(stdout), "write observation");
        }
        check_source(source,n,padding);
        cleanup();
        require(live_bytes == 0, "complete pass release");
    }
    require(allocations == 2 * OWNER_COUNT && releases == allocations, "exact owner release");
    printf("{\"record\":\"complete\",\"operations\":%" PRIu32 ",\"passes\":2,\"allocations\":%zu,\"releases\":%zu,\"live_bytes\":0,\"peak_dynamic_bytes\":%zu}\n",count,allocations,releases,peak_bytes);
    require(fflush(stdout)==0 && !ferror(stdout), "complete output flush");
    return 0;
}
