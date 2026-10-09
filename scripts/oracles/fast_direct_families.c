/* Direct-family test observations of unchanged facebook/zstd v1.5.7 Fast.
 * Reference: f8745da6ff1ad1e7bab384bd1f9d742439278e99.
 * Fixture setup, validation and serialization only; no copied Fast parser.
 */
#define ZSTD_STATIC_LINKING_ONLY
#include "zstd.h"
#include "compress/zstd_fast.h"
#include <inttypes.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define GUARD_BYTES 64u
#define LEADING_BYTES 64u
#define MAX_SOURCE_BYTES (2u * 1024u * 1024u)
#define MAX_BLOCK_BYTES (128u * 1024u)
#define MAX_TARGET_LENGTH 131072u
#define ARRAY_COUNT(a) (sizeof(a) / sizeof((a)[0]))

typedef struct { unsigned char* allocation; unsigned char* data; size_t bytes; } Guarded;
/* Binary envelope v2: magic8, id64, twelve LE32 scalars, ordered LE32
 * seed positions, then prefix+block source bytes, and exact EOF. */
#define MAX_SEEDS 1024u
typedef struct {
    char name[64];
    size_t prefix_bytes, block_bytes;
    unsigned window_log, hash_log, min_match, target_length, prefix_floor;
    U32 initial_repeats[3];
    U32 seed_count, seeds[MAX_SEEDS];
    unsigned char* source;
} Recipe;
typedef struct { size_t ll, ml; U32 off_base, distance; } Observation;

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
static void print_hex(const unsigned char* bytes, size_t count) {
    size_t i; putchar('"');
    for (i = 0; i < count; ++i) printf("%02x", bytes[i]);
    putchar('"');
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
static void print_table_changes(const U32* before, const U32* after, size_t count) {
    size_t i; int comma = 0; putchar('[');
    for (i = 0; i < count; ++i) if (before[i] != after[i]) {
        if (comma) putchar(',');
        comma = 1;
        printf("[%zu,%" PRIu32 ",%" PRIu32 "]", i, before[i], after[i]);
    }
    putchar(']');
}
static void print_table_fingerprint(const U32* table, size_t count) {
    uint64_t hash = UINT64_C(0xcbf29ce484222325); size_t i;
    for (i = 0; i < count; ++i) {
        unsigned byte;
        for (byte = 0; byte < 4; ++byte) {
            hash ^= (table[i] >> (8 * byte)) & 255; hash *= UINT64_C(0x100000001b3);
        }
    }
    printf("\"%016" PRIx64 "\"", hash);
}

static void execute_recipe(const Recipe* r, unsigned char padding) {
    const unsigned hash_log = r->hash_log, min_match = r->min_match;
    const unsigned key_bytes = min_match == 3 ? 4 : min_match;
    size_t source_bytes, suffix, arena_bytes, table_entries, sequence_capacity;
    Guarded arena, arena_copy, table_storage, before_storage, sequence_storage;
    Guarded literal_storage, code_storage, replay_storage, observation_storage;
    unsigned char *source, *base;
    U32 *table, *before, parser_repeats[3], wire_repeats[3];
    Observation* observations;
    SeqStore_t store; ZSTD_MatchState_t state;
    size_t i, trailing, count, literal_bytes, literal_cursor, cursor;
    active_case = r->name;
    require(r->block_bytes >= 7 && r->block_bytes <= MAX_BLOCK_BYTES, "direct block admission");
    require(r->window_log >= 10 && r->window_log <= 20, "window admission");
    require(r->window_log < sizeof(size_t) * CHAR_BIT, "window shift");
    require(r->block_bytes <= ((size_t)1 << r->window_log), "block exceeds selected window");
    require(r->target_length <= MAX_TARGET_LENGTH, "target length admission");
    source_bytes = add_size(r->prefix_bytes, r->block_bytes);
    require(source_bytes <= MAX_SOURCE_BYTES && source_bytes <= UINT32_MAX - 2, "source/index admission");
    require(hash_log >= 6 && hash_log <= 18, "hash admission");
    require(min_match >= 3 && min_match <= 7, "minimum-match admission");
    require(r->prefix_floor <= r->prefix_bytes, "prefix floor is after block start");
    require(r->seed_count <= MAX_SEEDS, "seed count admission");
    require(hash_log < sizeof(size_t) * CHAR_BIT, "hash shift");
    table_entries = (size_t)1 << hash_log;
    sequence_capacity = add_size(r->block_bytes / MINMATCH, 1);
    /* All pointer formation stays in one initialized allocation:
     * base=source-2, with another62 bytes before base. Thus iend-8 and iend-32
     * are within it even for these short blocks. Before cleanup Fast forms
     * ip0+128 and ip0+step+1. There are <=block_bytes forward advances, so step
     * <=target_length+2+block_bytes conservatively. Later pair, nextStep and
     * prefetch expressions add <=128 more bytes. The suffix below covers all
     * expressions beyond logical iend. Initialized slack is not logical input.
     */
    suffix = add_size(add_size(MAX_TARGET_LENGTH, MAX_BLOCK_BYTES), 512);
    arena_bytes = add_size(add_size(LEADING_BYTES, source_bytes), suffix);
    arena = guarded_new(arena_bytes); arena_copy = guarded_new(arena_bytes);
    memset(arena.data, padding, arena.bytes);
    source = arena.data + LEADING_BYTES; base = source - ZSTD_WINDOW_START_INDEX;
    memcpy(source, r->source, source_bytes);
    memcpy(arena_copy.data, arena.data, arena.bytes);
    require(GUARD_BYTES % _Alignof(U32) == 0 && GUARD_BYTES % _Alignof(SeqDef) == 0
            && GUARD_BYTES % _Alignof(Observation) == 0, "guard alignment");
    table_storage = guarded_new(multiply_size(table_entries, sizeof(U32)));
    before_storage = guarded_new(table_storage.bytes);
    table = (U32*)table_storage.data; before = (U32*)before_storage.data;
    sequence_storage = guarded_new(multiply_size(sequence_capacity, sizeof(SeqDef)));
    /* Guard follows permitted write slack, not logical maxNbLit. */
    literal_storage = guarded_new(add_size(r->block_bytes, WILDCOPY_OVERLENGTH));
    code_storage = guarded_new(multiply_size(sequence_capacity, 3));
    replay_storage = guarded_new(source_bytes);
    observation_storage = guarded_new(multiply_size(sequence_capacity, sizeof(Observation)));
    observations = (Observation*)observation_storage.data;
    memset(&state, 0, sizeof(state)); memset(&store, 0, sizeof(store));
    state.cParams.windowLog = r->window_log; state.cParams.chainLog = 6;
    state.cParams.hashLog = hash_log; state.cParams.searchLog = 1;
    state.cParams.minMatch = min_match; state.cParams.targetLength = r->target_length;
    state.cParams.strategy = ZSTD_fast;
    require(!ZSTD_isError(ZSTD_checkCParams(state.cParams)), "invalid C parameters");
    state.window.base = base; state.window.dictBase = base;
    state.window.dictLimit = r->prefix_floor + ZSTD_WINDOW_START_INDEX;
    state.window.lowLimit = r->prefix_floor + ZSTD_WINDOW_START_INDEX;
    state.window.nextSrc = source + source_bytes;
    state.hashTable = table; state.nextToUpdate = ZSTD_WINDOW_START_INDEX;
    /* loadedDictEnd, dictMatchState, optional tables and all other state zero. */
    store.sequencesStart = (SeqDef*)sequence_storage.data; store.litStart = literal_storage.data;
    store.llCode = code_storage.data; store.mlCode = code_storage.data + sequence_capacity;
    store.ofCode = code_storage.data + 2 * sequence_capacity;
    store.maxNbSeq = sequence_capacity; store.maxNbLit = r->block_bytes;
    ZSTD_resetSeqStore(&store);
    for (i = 0; i < r->seed_count; ++i) {
        size_t position = r->seeds[i], bucket;
        /* Each selected key belongs entirely to the prior logical source.
         * An eight-byte physical hash load for widths5..7 may cross the block
         * boundary but remains in the initialized arena, never another object.
         * Expired older entries are allowed; Fast rejects them by prefix bound.
         * Ordered seeds intentionally preserve collisions/last-write behavior. */
        require(position < r->prefix_bytes && key_bytes <= r->prefix_bytes - position,
                "seed selected key must be entirely before block start");
        bucket = ZSTD_hashPtr(source + position, hash_log, min_match == 3 ? 4 : min_match);
        require(bucket < table_entries, "seed bucket bounds");
        table[bucket] = (U32)(position + ZSTD_WINDOW_START_INDEX);
    }
    memcpy(before, table, table_storage.bytes);
    memcpy(parser_repeats, r->initial_repeats, sizeof(parser_repeats));
    memcpy(wire_repeats, r->initial_repeats, sizeof(wire_repeats));
    for (i = 0; i < 3; ++i) require(parser_repeats[i] > 0, "positive initial repeat");
    trailing = ZSTD_compressBlock_fast(&state, &store, parser_repeats,
                                     source + r->prefix_bytes, r->block_bytes);
    require(trailing <= r->block_bytes, "returned trailing length bounds");
    guarded_check(&arena); guarded_check(&table_storage); guarded_check(&sequence_storage);
    guarded_check(&literal_storage); guarded_check(&code_storage);
    require(memcmp(arena.data, arena_copy.data, arena.bytes) == 0, "source or padding changed");
    /* Unchanged parser received proven capacity; guards detect outer writes.
     * Returned pointers still belong to those arrays before differences below. */
    count = (size_t)(store.sequences - store.sequencesStart);
    literal_bytes = (size_t)(store.lit - store.litStart);
    require(count <= sequence_capacity && literal_bytes <= r->block_bytes, "returned store capacity");
    require(store.longLengthType >= ZSTD_llt_none && store.longLengthType <= ZSTD_llt_matchLength, "long length kind");
    require(store.longLengthType == ZSTD_llt_none || store.longLengthPos < count, "long length position");
    memcpy(replay_storage.data, source, r->prefix_bytes);
    literal_cursor = 0; cursor = r->prefix_bytes;
    for (i = 0; i < count; ++i) {
        SeqDef const* sequence = store.sequencesStart + i;
        ZSTD_SequenceLength c_length = ZSTD_getSequenceLength(&store, sequence);
        size_t ll = sequence->litLength, ml = add_size(sequence->mlBase, MINMATCH), byte;
        U32 distance;
        if (store.longLengthType != ZSTD_llt_none && store.longLengthPos == i) {
            if (store.longLengthType == ZSTD_llt_literalLength) ll = add_size(ll, 65536);
            else ml = add_size(ml, 65536);
        }
        require(ll == c_length.litLength && ml == c_length.matchLength, "independent long length differs");
        require(literal_cursor <= literal_bytes && ll <= literal_bytes - literal_cursor, "literal read bounds");
        require(cursor <= source_bytes && ll <= source_bytes - cursor, "literal output bounds");
        memcpy(replay_storage.data + cursor, store.litStart + literal_cursor, ll);
        cursor += ll; literal_cursor += ll;
        distance = resolve_and_update(wire_repeats, sequence->offBase, ll);
        require(distance <= cursor, "match before initialized logical prefix");
        /* Window/floor are semantic constraints independent of byte replay.
         * Do not impose the stricter block-end hash bound on repeat matches. */
        require(distance <= ((size_t)1 << r->window_log), "match exceeds selected window");
        require(cursor - distance >= r->prefix_floor, "match precedes retained prefix floor");
        require(ml >= MINMATCH && ml <= source_bytes - cursor, "match output bounds");
        for (byte = 0; byte < ml; ++byte) { replay_storage.data[cursor] = replay_storage.data[cursor - distance]; cursor++; }
        observations[i].ll = ll; observations[i].ml = ml;
        observations[i].off_base = sequence->offBase; observations[i].distance = distance;
    }
    require(literal_cursor == literal_bytes, "unconsumed stored literals");
    require(trailing == source_bytes - cursor, "last literal coverage");
    memcpy(replay_storage.data + cursor, source + cursor, trailing);
    require(memcmp(replay_storage.data, source, source_bytes) == 0, "independent replay differs");
    require(parser_repeats[0] == wire_repeats[0] && parser_repeats[1] == wire_repeats[1], "parser-used repeats differ");
    require(parser_repeats[2] == r->initial_repeats[2], "Fast changed unused parser repeat cell");
    for (i = 0; i < table_entries; ++i) {
        U32 position = table[i];
        if (position != 0) {
            require(position >= ZSTD_WINDOW_START_INDEX
                    && position - ZSTD_WINDOW_START_INDEX <= source_bytes - key_bytes,
                    "final table key is not wholly in initialized logical source");
            require(ZSTD_hashPtr(base + position, hash_log, key_bytes) == i, "final slot/key disagreement");
        }
    }
    printf("{\"record\":\"case\",\"id\":\"%s\",\"padding\":%u,", r->name, padding);
    printf("\"prefix_bytes\":%zu,\"block_bytes\":%zu,\"source_fnv1a64\":\"%016" PRIx64 "\",",
           r->prefix_bytes, r->block_bytes, fingerprint_bytes(source, source_bytes));
    printf("\"window_log\":%u,\"hash_log\":%u,\"min_match\":%u,\"key_bytes\":%u,\"target_length\":%u,",
           r->window_log, hash_log, min_match, key_bytes, r->target_length);
    printf("\"prefix_floor\":%u,\"initial_dict_limit\":%u,\"initial_low_limit\":%u,",
           r->prefix_floor, state.window.dictLimit, state.window.lowLimit);
    printf("\"seeds\":[");
    for (i = 0; i < r->seed_count; ++i) { if (i) putchar(','); printf("%" PRIu32, r->seeds[i]); }
    printf("],\"table_entries\":%zu,", table_entries);
    printf("\"physical_leading_bytes\":%u,\"physical_suffix_bytes\":%zu,\"sequence_capacity\":%zu,\"literal_slack_bytes\":%u,",
           LEADING_BYTES, suffix, sequence_capacity, WILDCOPY_OVERLENGTH);
    fputs("\"initial_repeats\":", stdout); print_repeats(r->initial_repeats);
    fputs(",\"parser_repeats\":", stdout); print_repeats(parser_repeats);
    fputs(",\"wire_repeats\":", stdout); print_repeats(wire_repeats);
    printf(",\"long_length_type\":%u,\"long_length_position\":%u,\"trailing_literals\":%zu,\"stored_literals_hex\":",
           (unsigned)store.longLengthType, (unsigned)store.longLengthPos, trailing);
    print_hex(store.litStart, literal_bytes); fputs(",\"sequences\":[", stdout);
    for (i = 0; i < count; ++i) {
        Observation const* s = observations + i;
        if (i) putchar(',');
        printf("{\"ll\":%zu,\"ml\":%zu,\"distance\":%" PRIu32 ",\"off_base\":%" PRIu32 ",\"intent\":\"%s\"}",
               s->ll, s->ml, s->distance, s->off_base, s->off_base <= 3 ? "repeat" : "offset");
    }
    fputs("],\"table_before_nonzero\":", stdout); print_initial_entries(before, table_entries);
    fputs(",\"table_changes\":", stdout); print_table_changes(before, table, table_entries);
    fputs(",\"table_before_le32_fnv1a64\":", stdout); print_table_fingerprint(before, table_entries);
    fputs(",\"table_after_le32_fnv1a64\":", stdout); print_table_fingerprint(table, table_entries);
    fputs(",\"replay_equal\":true,\"guards_intact\":true}\n", stdout);
    guarded_drop(&observation_storage); guarded_drop(&replay_storage); guarded_drop(&code_storage);
    guarded_drop(&literal_storage); guarded_drop(&sequence_storage); guarded_drop(&before_storage);
    guarded_drop(&table_storage); guarded_drop(&arena_copy); guarded_drop(&arena);
}

static U32 read_u32(FILE* file) {
    unsigned char b[4];
    require(fread(b, 1, sizeof(b), file) == sizeof(b), "truncated scalar");
    return (U32)b[0] | (U32)b[1] << 8 | (U32)b[2] << 16 | (U32)b[3] << 24;
}
static Recipe read_recipe(const char* path) {
    Recipe r; unsigned char magic[8]; size_t bytes, i, terminator = 64;
    FILE* file = fopen(path, "rb");
    require(file != NULL, "open case envelope"); memset(&r, 0, sizeof(r));
    require(fread(magic, 1, 8, file) == 8 && memcmp(magic, "ZFAST02\0", 8) == 0, "envelope magic");
    require(fread(r.name, 1, 64, file) == 64, "truncated case id");
    for (i = 0; i < 64; ++i) {
        unsigned char c = (unsigned char)r.name[i];
        if (c == 0 && terminator == 64) terminator = i;
        require((terminator < 64 && c == 0) || (terminator == 64 &&
                ((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-' || c == '_')),
                "bounded unescaped ASCII case id");
    }
    require(terminator > 0 && terminator < 64, "case id termination");
    r.prefix_bytes = read_u32(file); r.block_bytes = read_u32(file);
    r.window_log = read_u32(file); r.hash_log = read_u32(file);
    r.min_match = read_u32(file); r.target_length = read_u32(file);
    r.prefix_floor = read_u32(file);
    for (i = 0; i < 3; ++i) r.initial_repeats[i] = read_u32(file);
    r.seed_count = read_u32(file);
    require(read_u32(file) == 0, "reserved envelope scalar");
    require(r.seed_count <= MAX_SEEDS, "seed count envelope cap");
    bytes = add_size(r.prefix_bytes, r.block_bytes);
    require(bytes <= MAX_SOURCE_BYTES && bytes <= UINT32_MAX - 2, "source envelope cap");
    require(r.block_bytes >= 7 && r.block_bytes <= MAX_BLOCK_BYTES, "block envelope cap");
    for (i = 0; i < r.seed_count; ++i) r.seeds[i] = read_u32(file);
    r.source = (unsigned char*)malloc(bytes); require(r.source != NULL, "source envelope allocation");
    require(fread(r.source, 1, bytes, file) == bytes, "truncated source envelope");
    require(fgetc(file) == EOF && !ferror(file), "exact envelope EOF");
    require(fclose(file) == 0, "close envelope");
    return r;
}
int main(int argc, char** argv) {
    Recipe recipe; const U32 endian_probe = 1;
    require(argc == 2, "usage: fast_direct_families CASE_PACKET");
    recipe = read_recipe(argv[1]);
    printf("{\"record\":\"metadata\",\"schema_version\":2,\"upstream_revision\":\"f8745da6ff1ad1e7bab384bd1f9d742439278e99\","
           "\"entry_point\":\"ZSTD_compressBlock_fast\",\"observations\":\"sequences-and-final-table-state-not-chronological-trace\","
           "\"strategy\":\"fast\",\"chain_log\":6,\"search_log\":1,\"loaded_dict_end\":0,\"dict_match_state\":false,"
           "\"pointer_bits\":%zu,\"little_endian\":%s,\"recipe_count\":1,\"case_count\":2,\"sizeof_seqdef\":%zu,\"sizeof_match_state\":%zu}\n",
           sizeof(void*) * CHAR_BIT, *(const unsigned char*)&endian_probe ? "true" : "false", sizeof(SeqDef), sizeof(ZSTD_MatchState_t));
    execute_recipe(&recipe, 0x6a); execute_recipe(&recipe, 0xc3);
    require(fflush(stdout) == 0 && !ferror(stdout), "stdout write failure");
    fprintf(stderr, "Validated2 direct Fast observations for %s; no collector or frame campaign.\n", recipe.name);
    free(recipe.source); return 0;
}
