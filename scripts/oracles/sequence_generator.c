/* Independent bounded oracle for facebook/zstd v1.5.7 tests/seqgen.c.
 * The unchanged upstream generator is compiled separately and linked with the
 * pinned test library. No C implementation is linked into zstdoxide.
 * Copyright (c) zstdoxide contributors. BSD-3-Clause; see LICENSE.
 * Upstream seqgen and xxHash are covered by LICENSE.zstd's BSD option.
 *
 * `schedule`: write all 971 original zstreamtest.c:1551 request records as text:
 * kind value length request_xxh64 cumulative_xxh64 seed, then a TOTAL record.
 * `calls`: stdin is "ZSEQ0001", LE u32 seed/count, then count triples of LE u32
 * kind/value/capacity. stdout is "ZSQO0001", LE u32 count, then records of
 * length/remaining/seed/state/saved (u32), cumulative XXH64 (u64), and length
 * output bytes. At most 4096 calls, each with capacity at most 4096, are allowed.
 */
#include "seqgen.h"
#include "zstd.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define INITIAL_SEED 0x87654321u
#define GUARD 16u

static void require(int condition, const char* message) {
    if (!condition) { fprintf(stderr, "%s\n", message); exit(1); }
}

static SEQ_stream make_stream(unsigned seed) {
    SEQ_stream stream = SEQ_initStream(seed);
    /* C leaves inactive fields unspecified. Initialize them for deterministic
     * observations only; SEQ_gen overwrites each before it affects output. */
    stream.saved = 0;
    stream.bytesLeft = 0;
    return stream;
}

static uint32_t read_u32(void) {
    unsigned char bytes[4];
    require(fread(bytes, 1, 4, stdin) == 4, "truncated request");
    return (uint32_t)bytes[0] | (uint32_t)bytes[1] << 8 |
           (uint32_t)bytes[2] << 16 | (uint32_t)bytes[3] << 24;
}

static void write_number(uint64_t number, unsigned width) {
    unsigned char bytes[8];
    unsigned index;
    for (index = 0; index < width; ++index) bytes[index] = (unsigned char)(number >> (8 * index));
    require(fwrite(bytes, 1, width, stdout) == width, "response write failed");
}

static size_t generate(SEQ_stream* stream, SEQ_gen_type type, unsigned value,
                       unsigned char* guarded, size_t capacity, size_t* length) {
    SEQ_outBuffer output = {guarded + GUARD, capacity, 0};
    size_t pending, index;
    memset(guarded, 0xa5, capacity + 2 * GUARD);
    pending = SEQ_gen(stream, type, value, &output);
    require(output.pos <= capacity && pending <= UINT32_MAX, "generator bound");
    for (index = 0; index < GUARD; ++index)
        require(guarded[index] == 0xa5 && guarded[GUARD + capacity + index] == 0xa5, "generator canary");
    for (index = output.pos; index < capacity; ++index)
        require(guarded[GUARD + index] == 0xa5, "generator modified unused output");
    *length = output.pos;
    return pending;
}

static uint64_t request(SEQ_stream* stream, unsigned kind, unsigned value) {
    unsigned char guarded[1024 + 2 * GUARD];
    XXH64_state_t hash;
    uint64_t total = 0;
    size_t pending;
    XXH64_reset(&hash, 0);
    do {
        size_t length;
        pending = generate(stream, (SEQ_gen_type)kind, value, guarded, 1024, &length);
        total += length;
        require(total <= (1u << 24) + 128u, "schedule request output bound");
        XXH64_update(&hash, guarded + GUARD, length);
    } while (pending != 0);
    require(printf("%u %u %llu %016llx %016llx %08x\n", kind, value,
        (unsigned long long)total, (unsigned long long)XXH64_digest(&hash),
        (unsigned long long)SEQ_digest(stream), stream->seed) > 0, "schedule write");
    return total;
}

int main(int argc, char** argv) {
    require(ZSTD_versionNumber() == 10507, "expected zstd v1.5.7");
    require(argc == 2, "expected schedule or calls mode");
    if (strcmp(argv[1], "schedule") == 0) {
        SEQ_stream stream = make_stream(INITIAL_SEED);
        unsigned value, kind, count = 0;
        uint64_t total = 0;
        for (value = 0; value < 300; ++value)
            for (kind = 0; kind < 3; ++kind) { total += request(&stream, kind, value); ++count; }
        for (value = 1u << 8; value < 1u << 17; value <<= 1)
            for (kind = 0; kind < 3; ++kind) {
                total += request(&stream, kind, value);
                total += request(&stream, kind, value + (value >> 2)); count += 2;
            }
        for (value = 8; value <= 24; ++value) { total += request(&stream, 2, (1u << value) - 1); ++count; }
        require(count == 971 && total == 34639743, "complete original schedule size");
        require(printf("TOTAL %u %llu %016llx %08x\n", count, (unsigned long long)total,
            (unsigned long long)SEQ_digest(&stream), stream.seed) > 0, "total write");
    } else if (strcmp(argv[1], "calls") == 0) {
        unsigned char magic[8], guarded[4096 + 2 * GUARD];
        unsigned seed, count, index, active_kind = 0, active_value = 0;
        size_t pending = 0;
        SEQ_stream stream;
        require(fread(magic, 1, 8, stdin) == 8 && memcmp(magic, "ZSEQ0001", 8) == 0, "request magic");
        seed = read_u32(); count = read_u32();
        require(count > 0 && count <= 4096, "call count bound");
        stream = make_stream(seed);
        require(fwrite("ZSQO0001", 1, 8, stdout) == 8, "response header");
        write_number(count, 4);
        for (index = 0; index < count; ++index) {
            unsigned kind = read_u32(), value = read_u32(), capacity = read_u32();
            size_t length;
            require(kind < 3 && capacity <= 4096, "call kind or capacity");
            require(pending == 0 || (kind == active_kind && value == active_value), "changed unfinished request");
            active_kind = kind; active_value = value;
            pending = generate(&stream, (SEQ_gen_type)kind, value, guarded, capacity, &length);
            write_number(length, 4); write_number(pending, 4); write_number(stream.seed, 4);
            write_number((unsigned)stream.state, 4); write_number(stream.saved, 4);
            write_number(SEQ_digest(&stream), 8);
            require(fwrite(guarded + GUARD, 1, length, stdout) == length, "response bytes");
        }
        require(fgetc(stdin) == EOF && !ferror(stdin), "trailing input or read error");
    } else { require(0, "unknown mode"); }
    require(fflush(stdout) == 0, "response flush failed");
    return 0;
}
