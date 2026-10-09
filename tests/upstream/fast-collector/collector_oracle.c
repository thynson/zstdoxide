/* Bounded public Fast sequence-collector corroboration; test oracle only.
 * Unchanged facebook/zstd v1.5.7, f8745da6ff1ad1e7bab384bd1f9d742439278e99.
 * Collector candidate publication is distinct from ordinary frame selection.
 */
#define ZSTD_STATIC_LINKING_ONLY
#define ZSTD_DISABLE_DEPRECATE_WARNINGS
#include "zstd.h"
#include "zstd_errors.h"
#include "compress/zstd_fast.h"
#include <inttypes.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define GUARD_BYTES 64u
#define LEADING_BYTES 64u
#define MAX_SOURCE_BYTES (512u * 1024u)
#define MAX_BLOCK_BYTES (128u * 1024u)
#define MAX_TARGET_LENGTH 131072u
#define ARRAY_COUNT(a) (sizeof(a) / sizeof((a)[0]))
typedef struct { unsigned char* allocation; unsigned char* data; size_t bytes; } Guarded;
/* magic8,id64,eight LE32: source,window,hash,minMatch,target,capacityKind,
 * capacityValue,reservedZero; source bytes; exact EOF. */
typedef struct {
    char name[64]; unsigned source_bytes, window_log, hash_log, min_match, target;
    unsigned capacity_kind, capacity_value; unsigned char* source;
} Recipe;
typedef struct { size_t ll, ml; U32 off_base, distance; } Observation;
typedef struct { U32 slot, before, after; } Change;
typedef struct {
    size_t start, bytes, first, count, trailing, change_start, changes;
    unsigned long_type, long_position;
    U32 parser_before[3], parser_after[3], wire_after[3];
    uint64_t table_before, table_after;
} Block;
static const char* active_case = "setup";

static void require(int valid, const char* reason) {
    if (!valid) { fprintf(stderr, "fast oracle [%s]: %s\n", active_case, reason); exit(1); }
}
static size_t add_size(size_t a, size_t b) {
    require(b <= SIZE_MAX - a, "size addition overflow"); return a + b;
}
static size_t multiply_size(size_t a, size_t b) {
    require(a == 0 || b <= SIZE_MAX / a, "size multiplication overflow"); return a * b;
}
static Guarded guarded_new(size_t bytes) {
    Guarded result;
    size_t total = add_size(add_size(GUARD_BYTES, bytes), GUARD_BYTES);
    result.allocation = (unsigned char*)malloc(total);
    require(result.allocation != NULL, "guarded allocation failed");
    result.data = result.allocation + GUARD_BYTES; result.bytes = bytes;
    memset(result.allocation, 0xd7, GUARD_BYTES);
    memset(result.data, 0, bytes);
    memset(result.data + bytes, 0x9b, GUARD_BYTES);
    return result;
}
static void guarded_check(const Guarded* b) {
    size_t i;
    for (i = 0; i < GUARD_BYTES; ++i) {
        require(b->allocation[i] == 0xd7, "leading allocation guard changed");
        require(b->data[b->bytes + i] == 0x9b, "trailing allocation guard changed");
    }
}
static void guarded_drop(Guarded* b) {
    guarded_check(b); free(b->allocation); b->allocation = NULL; b->data = NULL;
}
static uint64_t fingerprint_bytes(const unsigned char* data, size_t bytes) {
    uint64_t hash = UINT64_C(0xcbf29ce484222325); size_t i;
    for (i = 0; i < bytes; ++i) { hash ^= data[i]; hash *= UINT64_C(0x100000001b3); }
    return hash;
}
/* Independent full wire replay, without ZSTD_updateRep/copyBlockSequences.
 * The expanded LL selects the zero-literal rule, never the truncated U16. */
static U32 resolve_and_update(U32 repeats[3], U32 off_base, size_t ll) {
    U32 a = repeats[0], b = repeats[1], c = repeats[2], distance;
    require(off_base != 0 && a != 0 && b != 0 && c != 0, "zero offBase or saved repeat");
    if (off_base > 3) {
        distance = off_base - 3; repeats[2] = b; repeats[1] = a; repeats[0] = distance;
    } else if (ll != 0 && off_base == 1) {
        distance = a;
    } else if ((ll != 0 && off_base == 2) || (ll == 0 && off_base == 1)) {
        distance = b; repeats[0] = b; repeats[1] = a;
    } else {
        if (ll == 0 && off_base == 3) {
            require(a > 1, "rep1-minus-one must remain positive"); distance = a - 1;
        } else distance = c;
        repeats[2] = b; repeats[1] = a; repeats[0] = distance;
    }
    require(distance != 0, "zero resolved distance"); return distance;
}
static void print_repeats(const U32 r[3]) {
    printf("[%" PRIu32 ",%" PRIu32 ",%" PRIu32 "]", r[0], r[1], r[2]);
}
static void print_initial_entries(const U32* table, size_t count) {
    size_t i; int comma = 0; putchar('[');
    for (i = 0; i < count; ++i) if (table[i]) {
        if (comma) putchar(',');
        comma = 1;
        printf("[%zu,%" PRIu32 "]", i, table[i]);
    }
    putchar(']');
}
static uint64_t table_fingerprint(const U32* table, size_t count) {
    uint64_t hash = UINT64_C(0xcbf29ce484222325); size_t i;
    for (i = 0; i < count; ++i) {
        unsigned byte;
        for (byte = 0; byte < 4; ++byte) {
            hash ^= (table[i] >> (8 * byte)) & 255; hash *= UINT64_C(0x100000001b3);
        }
    }
    return hash;
}
static void check_api(size_t result, const char* reason) {
    if (ZSTD_isError(result)) fprintf(stderr, "%s: %s\n", reason, ZSTD_getErrorName(result));
    require(!ZSTD_isError(result), reason);
}
static void set_parameter(ZSTD_CCtx* c, ZSTD_cParameter parameter, int value) {
    int observed = INT_MIN;
    check_api(ZSTD_CCtx_setParameter(c, parameter, value), "public setter");
    check_api(ZSTD_CCtx_getParameter(c, parameter, &observed), "public getter");
    require(observed == value, "requested getter disagrees with explicit setter");
}
static void print_parameters(ZSTD_compressionParameters p) {
    printf("{\"window_log\":%u,\"hash_log\":%u,\"chain_log\":%u,\"search_log\":%u,\"min_match\":%u,\"target\":%u,\"strategy\":%u}",
           p.windowLog, p.hashLog, p.chainLog, p.searchLog, p.minMatch, p.targetLength, (unsigned)p.strategy);
}
static void print_public_rows(const ZSTD_Sequence* rows, size_t count) {
    size_t i; putchar('[');
    for (i = 0; i < count; ++i) {
        if (i) putchar(',');
        printf("[%u,%u,%u,%u]", rows[i].litLength, rows[i].matchLength, rows[i].offset, rows[i].rep);
    }
    putchar(']');
}
static void execute(const Recipe* r, unsigned char padding) {
    size_t source_bytes = r->source_bytes, arena_bytes, bound, capacity, result, i;
    Guarded arena, arena_copy, output;
    unsigned char *source;
    ZSTD_CCtx* c;
    ZSTD_Sequence* rows;
    ZSTD_compressionParameters requested;
    active_case = r->name;
    /* Source has initialized leading and suffix slack covering Fast's pointer
     * formation (iend-8, ip0+128, target+block growth and prefetch+128), as in
     * the separately reviewed direct-family helper. All pointers stay in one
     * allocation; expired logical prefix remains physically initialized. */
    arena_bytes = add_size(add_size(LEADING_BYTES, source_bytes),
                           add_size(add_size(MAX_TARGET_LENGTH, MAX_BLOCK_BYTES), 512));
    arena = guarded_new(arena_bytes); arena_copy = guarded_new(arena_bytes);
    memset(arena.data, padding, arena.bytes); source = arena.data + LEADING_BYTES;
    memcpy(source, r->source, source_bytes); memcpy(arena_copy.data, arena.data, arena.bytes);
    bound = ZSTD_sequenceBound(source_bytes);
    require(bound <= 176000, "public sequence bound admission");
    capacity = r->capacity_kind ? r->capacity_value : bound;
    require(capacity <= bound, "requested capacity exceeds bounded envelope");
    require(GUARD_BYTES % _Alignof(ZSTD_Sequence) == 0, "public output alignment");
    output = guarded_new(multiply_size(capacity, sizeof(ZSTD_Sequence)));
    /* Delimiter.rep is not written by pinned C, so zero-initialize ALL output.
     * guarded_new does so even when the permitted payload is zero bytes. */
    rows = (ZSTD_Sequence*)output.data;
    c = ZSTD_createCCtx(); require(c != NULL, "fresh CCtx allocation");
    set_parameter(c, ZSTD_c_windowLog, (int)r->window_log);
    set_parameter(c, ZSTD_c_hashLog, (int)r->hash_log);
    set_parameter(c, ZSTD_c_chainLog, 6); set_parameter(c, ZSTD_c_searchLog, 1);
    set_parameter(c, ZSTD_c_minMatch, (int)r->min_match);
    set_parameter(c, ZSTD_c_targetLength, (int)r->target);
    set_parameter(c, ZSTD_c_strategy, ZSTD_fast);
    set_parameter(c, ZSTD_c_nbWorkers, 0); set_parameter(c, ZSTD_c_targetCBlockSize, 0);
    set_parameter(c, ZSTD_c_enableLongDistanceMatching, ZSTD_ps_disable);
    set_parameter(c, ZSTD_c_blockSplitterLevel, 1);
    set_parameter(c, ZSTD_c_splitAfterSequences, ZSTD_ps_disable);
    set_parameter(c, ZSTD_c_useRowMatchFinder, ZSTD_ps_disable);
    set_parameter(c, ZSTD_c_checksumFlag, 0); set_parameter(c, ZSTD_c_contentSizeFlag, 1);
    set_parameter(c, ZSTD_c_srcSizeHint, 0); set_parameter(c, ZSTD_c_maxBlockSize, MAX_BLOCK_BYTES);
    requested = c->requestedParams.cParams;
    result = ZSTD_generateSequences(c, rows, capacity, source, source_bytes);
    guarded_check(&arena); guarded_check(&output);
    require(memcmp(arena.data, arena_copy.data, arena.bytes) == 0, "collector changed source/slack");
    /* Emit the original API result before private corroboration. Any later
     * assertion diagnostic still leaves the exact original public rows. Error
     * paths deliberately do not read output rows or ANY private CCtx state. */
    printf("{\"type\":\"public\",\"id\":\"%s\",\"padding\":%u,\"source_bytes\":%zu,\"source_fnv\":\"%016" PRIx64 "\",\"bound\":%zu,\"capacity\":%zu,\"requested\":",
           r->name, padding, source_bytes, fingerprint_bytes(source, source_bytes), bound, capacity);
    print_parameters(requested);
    printf(",\"error\":%u,\"error_name\":\"%s\",\"rows\":", (unsigned)ZSTD_getErrorCode(result), ZSTD_getErrorName(result));
    if (ZSTD_isError(result)) printf("null");
    else { require(result <= capacity, "successful row count exceeds caller output"); print_public_rows(rows, result); }
    printf("}\n"); fflush(stdout);
    if (!ZSTD_isError(result) && source_bytes != 0) {
        ZSTD_compressionParameters actual = c->appliedParams.cParams;
        const ZSTD_MatchState_t* observed_state = &c->blockState.matchState;
        size_t window, block_max = c->blockSizeMax, table_count, block_capacity;
        size_t block_count = 0, row_cursor = 0, change_count = 0, cursor = 0;
        size_t global_wire_mismatches = 0, direct_mismatches = 0;
        Guarded table_mem, before_mem, seq_mem, lit_mem, code_mem, replay_mem, blocks_mem, changes_mem;
        Guarded direct_mem;
        U32 *table, *before, parser[3] = {1,4,8}, global_wire[3] = {1,4,8};
        Block* blocks; Change* changes; Observation* direct;
        SeqStore_t store; ZSTD_MatchState_t state;
        unsigned width;
        /* Validate every dimension before shifts, pointer normalization or
         * allocation. Applied settings may differ from the public requests. */
        require(!ZSTD_isError(ZSTD_checkCParams(actual)), "applied C parameters invalid");
        require(actual.strategy == ZSTD_fast && actual.windowLog >= 10 && actual.windowLog <= 20,
                "applied strategy/window outside direct domain");
        require(actual.hashLog >= 6 && actual.hashLog <= 18 && actual.chainLog >= 6 && actual.chainLog <= 20
                && actual.searchLog >= 1 && actual.searchLog <= 20, "applied table/search domain");
        require(actual.minMatch >= 3 && actual.minMatch <= 7 && actual.targetLength <= MAX_TARGET_LENGTH,
                "applied match/target domain");
        require(actual.windowLog < sizeof(size_t)*CHAR_BIT && actual.hashLog < sizeof(size_t)*CHAR_BIT,
                "applied shift widths");
        window = (size_t)1 << actual.windowLog; table_count = (size_t)1 << actual.hashLog;
        width = actual.minMatch == 3 ? 4 : actual.minMatch;
        require(block_max > 0 && block_max <= MAX_BLOCK_BYTES && block_max <= window, "effective block size domain");
        require(c->appliedParams.nbWorkers == 0 && c->appliedParams.targetCBlockSize == 0
                && c->appliedParams.ldmParams.enableLdm == ZSTD_ps_disable
                && c->appliedParams.preBlockSplitter_level == 1
                && c->appliedParams.postBlockSplitter == ZSTD_ps_disable
                && c->appliedParams.useRowMatchFinder == ZSTD_ps_disable
                && c->appliedParams.maxBlockSize == MAX_BLOCK_BYTES, "applied auxiliary modes");
        require(c->appliedParams.fParams.checksumFlag == 0 && c->appliedParams.fParams.contentSizeFlag == 1
                && c->appliedParams.srcSizeHint == 0, "applied frame/source hints");
        require(observed_state->window.base == source - ZSTD_WINDOW_START_INDEX,
                "collector base does not normalize to owned initialized source-minus-two");
        require(observed_state->hashTable != NULL && c->blockState.prevCBlock != NULL, "initialized collector owner pointers");
        require(observed_state->cParams.hashLog == actual.hashLog && observed_state->cParams.minMatch == actual.minMatch,
                "applied match-state dimensions");
        block_capacity = add_size(source_bytes / block_max, 1);
        require(block_capacity <= 513, "bounded direct block count");
        require(GUARD_BYTES % _Alignof(Block) == 0 && GUARD_BYTES % _Alignof(SeqDef) == 0
                && GUARD_BYTES % _Alignof(Change) == 0 && GUARD_BYTES % _Alignof(Observation) == 0, "direct scratch alignment");
        table_mem = guarded_new(multiply_size(table_count, sizeof(U32))); before_mem = guarded_new(table_mem.bytes);
        seq_mem = guarded_new(multiply_size(add_size(block_max / MINMATCH, 1), sizeof(SeqDef)));
        lit_mem = guarded_new(add_size(block_max, WILDCOPY_OVERLENGTH));
        code_mem = guarded_new(multiply_size(add_size(block_max / MINMATCH, 1), 3));
        replay_mem = guarded_new(source_bytes); blocks_mem = guarded_new(multiply_size(block_capacity, sizeof(Block)));
        /* Every changed slot holds a selected key starting inside its block.
         * Distinct changes therefore cannot exceed source_bytes overall. */
        changes_mem = guarded_new(multiply_size(source_bytes, sizeof(Change)));
        direct_mem = guarded_new(multiply_size(bound, sizeof(Observation)));
        table = (U32*)table_mem.data; before = (U32*)before_mem.data;
        blocks = (Block*)blocks_mem.data; changes = (Change*)changes_mem.data; direct = (Observation*)direct_mem.data;
        memset(&state, 0, sizeof(state)); memset(&store, 0, sizeof(store));
        state.cParams = actual; state.hashTable = table; state.nextToUpdate = ZSTD_WINDOW_START_INDEX;
        state.window.base = source - ZSTD_WINDOW_START_INDEX; state.window.dictBase = state.window.base;
        state.window.nextSrc = source + source_bytes;
        store.sequencesStart = (SeqDef*)seq_mem.data; store.litStart = lit_mem.data;
        store.maxNbSeq = add_size(block_max / MINMATCH, 1); store.maxNbLit = block_max;
        store.llCode = code_mem.data; store.mlCode = code_mem.data + store.maxNbSeq;
        store.ofCode = code_mem.data + 2 * store.maxNbSeq;
        while (cursor < source_bytes) {
            size_t block_start = cursor, block_bytes = source_bytes-cursor, floor;
            size_t trailing, count, literal_cursor = 0, literal_bytes, j;
            U32 local_wire[3]; Block* b;
            if (block_bytes > block_max) block_bytes = block_max;
            require(block_bytes >= 7, "successful collector exposed unsupported tiny candidate");
            require(block_count < block_capacity, "direct block output capacity"); b = blocks + block_count++;
            b->start = block_start; b->bytes = block_bytes; b->first = row_cursor;
            floor = block_start > window ? block_start - window : 0;
            state.window.lowLimit = (U32)(floor + ZSTD_WINDOW_START_INDEX);
            state.window.dictLimit = (U32)(floor + ZSTD_WINDOW_START_INDEX);
            memcpy(b->parser_before, parser, sizeof(parser)); memcpy(local_wire, parser, sizeof(parser));
            memcpy(before, table, table_mem.bytes); b->table_before = table_fingerprint(table, table_count);
            ZSTD_resetSeqStore(&store);
            trailing = ZSTD_compressBlock_fast(&state, &store, parser, source+block_start, block_bytes);
            require(trailing <= block_bytes, "direct trailing bound");
            guarded_check(&table_mem); guarded_check(&seq_mem); guarded_check(&lit_mem); guarded_check(&code_mem);
            require(memcmp(arena.data, arena_copy.data, arena.bytes) == 0, "direct changed source/slack");
            count = (size_t)(store.sequences-store.sequencesStart); literal_bytes = (size_t)(store.lit-store.litStart);
            require(count <= store.maxNbSeq && literal_bytes <= block_bytes, "direct returned scratch capacity");
            require(store.longLengthType >= ZSTD_llt_none && store.longLengthType <= ZSTD_llt_matchLength
                    && (store.longLengthType == ZSTD_llt_none || store.longLengthPos < count), "direct long length metadata");
            b->count = count; b->trailing = trailing; b->long_type = store.longLengthType;
            b->long_position = store.longLengthType == ZSTD_llt_none ? 0 : store.longLengthPos;
            for (j = 0; j < count; ++j) {
                const SeqDef* sequence = store.sequencesStart+j;
                ZSTD_SequenceLength c_length = ZSTD_getSequenceLength(&store, sequence);
                size_t ll = sequence->litLength, ml = add_size(sequence->mlBase, MINMATCH), k;
                U32 distance, global_distance; const ZSTD_Sequence* original;
                if (store.longLengthType != ZSTD_llt_none && store.longLengthPos == j) {
                    if (store.longLengthType == ZSTD_llt_literalLength) ll = add_size(ll, 65536);
                    else ml = add_size(ml, 65536);
                }
                require(ll == c_length.litLength && ml == c_length.matchLength, "expanded direct length mismatch");
                require(row_cursor < result && row_cursor < bound, "collector/direct row boundary");
                original = rows+row_cursor;
                require(ll <= block_start+block_bytes-cursor && ll <= literal_bytes-literal_cursor, "literal extents");
                require(memcmp(store.litStart+literal_cursor, source+cursor, ll) == 0, "direct stored literal bytes");
                memcpy(replay_mem.data+cursor, store.litStart+literal_cursor, ll); cursor += ll; literal_cursor += ll;
                distance = resolve_and_update(local_wire, sequence->offBase, ll);
                global_distance = resolve_and_update(global_wire, sequence->offBase, ll);
                if (global_distance != distance) ++global_wire_mismatches;
                require(distance <= cursor && distance <= window && cursor-distance >= floor, "direct wire retained/window bound");
                require(ml <= block_start+block_bytes-cursor, "match extent");
                for (k = 0; k < ml; ++k) replay_mem.data[cursor+k] = replay_mem.data[cursor+k-distance];
                require(memcmp(replay_mem.data+cursor, source+cursor, ml) == 0, "direct full byte reconstruction"); cursor += ml;
                direct[row_cursor].ll = ll; direct[row_cursor].ml = ml;
                direct[row_cursor].off_base = sequence->offBase; direct[row_cursor].distance = distance;
                if (original->litLength != ll || original->matchLength != ml || original->offset != distance
                    || original->rep != (sequence->offBase <= 3 ? sequence->offBase : 0)) ++direct_mismatches;
                ++row_cursor;
            }
            require(literal_cursor == literal_bytes && trailing == block_start+block_bytes-cursor, "direct literals/trailing accounting");
            require(row_cursor < result && row_cursor < bound, "collector delimiter capacity");
            if (rows[row_cursor].litLength != trailing || rows[row_cursor].matchLength != 0
                || rows[row_cursor].offset != 0 || rows[row_cursor].rep != 0) ++direct_mismatches;
            direct[row_cursor].ll = trailing;
            memcpy(replay_mem.data+cursor, source+cursor, trailing); cursor += trailing; ++row_cursor;
            require(parser[0] == local_wire[0] && parser[1] == local_wire[1]
                    && parser[2] == b->parser_before[2], "parser first-two/full replay distinction");
            memcpy(b->parser_after, parser, sizeof(parser)); memcpy(b->wire_after, local_wire, sizeof(parser));
            b->change_start = change_count;
            for (i = 0; i < table_count; ++i) if (before[i] != table[i]) {
                require(change_count < source_bytes, "aggregate table delta capacity");
                require(table[i] >= ZSTD_WINDOW_START_INDEX && table[i]-ZSTD_WINDOW_START_INDEX >= block_start
                        && table[i]-ZSTD_WINDOW_START_INDEX <= block_start+block_bytes-width, "changed key must belong to current complete block");
                changes[change_count].slot = (U32)i; changes[change_count].before = before[i]; changes[change_count].after = table[i];
                ++change_count;
            }
            b->changes = change_count-b->change_start; b->table_after = table_fingerprint(table, table_count);
        }
        require(row_cursor == result && memcmp(replay_mem.data, source, source_bytes) == 0, "complete direct row/source coverage");
        /* Public raw offsets are replayed independently too. A mismatching
         * offBase intent is recorded separately and never repairs an API row. */
        { size_t public_cursor = 0, block_start = 0; U32 wire[3] = {1,4,8}; size_t wire_mismatches = 0;
          for (i = 0; i < result; ++i) {
            const ZSTD_Sequence* q = rows+i; size_t k, floor = block_start > window ? block_start-window : 0;
            require(q->litLength <= source_bytes-public_cursor, "public literal bounds");
            memcpy(replay_mem.data+public_cursor, source+public_cursor, q->litLength); public_cursor += q->litLength;
            if (q->offset == 0 && q->matchLength == 0) {
                size_t expected = source_bytes-block_start; if (expected > block_max) expected = block_max;
                require(q->rep == 0 && public_cursor-block_start == expected, "public zero delimiter/block boundary");
                block_start = public_cursor; continue;
            }
            require(q->offset > 0 && q->matchLength >= MINMATCH && q->rep <= 3, "public match fields");
            require(q->offset <= public_cursor && q->offset <= window && public_cursor-q->offset >= floor, "public retained/window offset bound");
            require(q->matchLength <= source_bytes-public_cursor, "public match bounds");
            if (resolve_and_update(wire, q->rep ? q->rep : q->offset+3, q->litLength) != q->offset) ++wire_mismatches;
            for (k = 0; k < q->matchLength; ++k) replay_mem.data[public_cursor+k] = replay_mem.data[public_cursor+k-q->offset];
            require(memcmp(replay_mem.data+public_cursor, source+public_cursor, q->matchLength) == 0, "public raw-offset source reconstruction");
            public_cursor += q->matchLength;
          }
          require(public_cursor == source_bytes && block_start == source_bytes, "public complete delimiter coverage");
          printf("{\"type\":\"corroboration\",\"id\":\"%s\",\"padding\":%u,\"applied\":", r->name, padding); print_parameters(actual);
          printf(",\"max_block_parameter\":%zu,\"effective_block_bytes\":%zu,\"direct_mismatches\":%zu,\"public_wire_mismatches\":%zu,\"direct_global_wire_mismatches\":%zu,\"public_wire_after\":", c->appliedParams.maxBlockSize, block_max, direct_mismatches, wire_mismatches, global_wire_mismatches); print_repeats(wire);
        }
        printf(",\"parser_after\":"); print_repeats(parser);
        printf(",\"collector_parser_after\":"); print_repeats(c->blockState.prevCBlock->rep);
        printf(",\"parser_first_two_equal\":%s,\"tables_equal\":%s,\"collector_final_table\":",
               parser[0] == c->blockState.prevCBlock->rep[0] && parser[1] == c->blockState.prevCBlock->rep[1] ? "true":"false",
               memcmp(table, observed_state->hashTable, table_mem.bytes) == 0 ? "true":"false");
        print_initial_entries(observed_state->hashTable, table_count);
        printf(",\"collector_table_fnv\":\"%016" PRIx64 "\",\"direct_rows\":[", table_fingerprint(observed_state->hashTable, table_count));
        for (i = 0; i < result; ++i) {
            if (i) putchar(',');
            printf("[%zu,%zu,%" PRIu32 ",%" PRIu32 "]", direct[i].ll, direct[i].ml, direct[i].distance, direct[i].off_base);
        }
        printf("],\"blocks\":[");
        for (i = 0; i < block_count; ++i) {
            Block* b = blocks+i; size_t j;
            if (i) putchar(',');
            printf("{\"start\":%zu,\"bytes\":%zu,\"first_row\":%zu,\"sequence_count\":%zu,\"trailing\":%zu,\"long_type\":%u,\"long_position\":%u,\"parser_before\":", b->start,b->bytes,b->first,b->count,b->trailing,b->long_type,b->long_position); print_repeats(b->parser_before);
            printf(",\"parser_after\":"); print_repeats(b->parser_after); printf(",\"wire_after\":"); print_repeats(b->wire_after);
            printf(",\"table_before_fnv\":\"%016" PRIx64 "\",\"table_after_fnv\":\"%016" PRIx64 "\",\"changes\":[", b->table_before,b->table_after);
            for (j = 0; j < b->changes; ++j) { Change* change = changes+b->change_start+j; if (j) putchar(','); printf("[%" PRIu32 ",%" PRIu32 ",%" PRIu32 "]", change->slot,change->before,change->after); }
            printf("]}");
        }
        printf("]}\n"); fflush(stdout);
        guarded_drop(&direct_mem); guarded_drop(&changes_mem); guarded_drop(&blocks_mem); guarded_drop(&replay_mem);
        guarded_drop(&code_mem); guarded_drop(&lit_mem); guarded_drop(&seq_mem); guarded_drop(&before_mem); guarded_drop(&table_mem);
    }
    check_api(ZSTD_freeCCtx(c), "CCtx free");
    guarded_drop(&output); guarded_drop(&arena_copy); guarded_drop(&arena);
}
static U32 read_u32(const unsigned char* p) {
    return (U32)p[0] | ((U32)p[1]<<8) | ((U32)p[2]<<16) | ((U32)p[3]<<24);
}
static Recipe read_recipe(const char* path) {
    unsigned char header[104]; Recipe r; FILE* file = fopen(path,"rb"); size_t i;
    require(file != NULL, "packet open"); memset(&r,0,sizeof(r));
    require(fread(header,1,sizeof(header),file) == sizeof(header), "packet fixed header");
    require(memcmp(header,"ZCOLL01\0",8) == 0, "packet magic");
    memcpy(r.name,header+8,64); require(r.name[0] != 0 && r.name[63] == 0, "packet id extent");
    for (i = 0; i < 64; ++i) {
        unsigned char c = (unsigned char)r.name[i];
        if (!c) { while (i < 64) { require(r.name[i] == 0,"packet id padding"); ++i; } break; }
        require((c>='a'&&c<='z')||(c>='0'&&c<='9')||c=='-'||c=='_',"packet id alphabet");
    }
    active_case=r.name;
    r.source_bytes=read_u32(header+72); r.window_log=read_u32(header+76); r.hash_log=read_u32(header+80);
    r.min_match=read_u32(header+84); r.target=read_u32(header+88); r.capacity_kind=read_u32(header+92); r.capacity_value=read_u32(header+96);
    require(read_u32(header+100)==0,"packet reserved field");
    require(r.source_bytes<=MAX_SOURCE_BYTES && r.window_log>=10 && r.window_log<=20,"packet source/window domain");
    require(r.hash_log>=6&&r.hash_log<=18&&r.min_match>=3&&r.min_match<=7&&r.target<=MAX_TARGET_LENGTH,"packet parser domain");
    require(r.capacity_kind<=1 && (r.capacity_kind || r.capacity_value==0),"packet capacity kind/value");
    require(r.capacity_value<=ZSTD_sequenceBound(r.source_bytes),"packet capacity bound");
    r.source=(unsigned char*)malloc(add_size(r.source_bytes,1)); require(r.source!=NULL,"packet source allocation");
    require(fread(r.source,1,r.source_bytes,file)==r.source_bytes,"packet truncated source");
    require(fgetc(file)==EOF&&!ferror(file),"packet trailing bytes/read error"); require(fclose(file)==0,"packet close");
    active_case="setup"; return r;
}
int main(int argc,char** argv) {
    Recipe r; unsigned endian=1;
    require(argc==2,"usage: collector_oracle packet"); require(ZSTD_versionNumber()==10507,"pinned library version");
    require(ZSTD_WINDOW_START_INDEX==2 && MINMATCH==3 && WILDCOPY_OVERLENGTH==32,"pinned setup constants");
    printf("{\"type\":\"metadata\",\"revision\":\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\",\"version\":%u,\"size_t_bytes\":%zu,\"sequence_bytes\":%zu,\"little_endian\":%s,\"source_cap\":%u,\"block_cap\":%u,\"suffix_bytes\":%u,\"direct_next_to_update\":2}\n", ZSTD_versionNumber(),sizeof(size_t),sizeof(ZSTD_Sequence),*(unsigned char*)&endian?"true":"false",MAX_SOURCE_BYTES,MAX_BLOCK_BYTES,MAX_TARGET_LENGTH+MAX_BLOCK_BYTES+512);
    r=read_recipe(argv[1]); execute(&r,106); execute(&r,195); free(r.source); return 0;
}
