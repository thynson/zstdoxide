/* Offline oracle only; never compiled or linked into the Rust crate.
 * Contracts derive from facebook/zstd v1.5.7 dictionary and prefix tests.
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 * Copyright (c) zstdoxide contributors. BSD-3-Clause; see LICENSE.zstd.
 */
#define ZSTD_STATIC_LINKING_ONLY
#include "zstd.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CAPACITY (1024 * 1024)

static void require(int valid, const char* message) {
    if (!valid) { fprintf(stderr, "%s\n", message); exit(2); }
}

static size_t read_file(const char* path, unsigned char* data) {
    FILE* file = fopen(path, "rb");
    size_t size;
    require(file != NULL, "cannot open oracle input");
    size = fread(data, 1, CAPACITY, file);
    require(!ferror(file) && feof(file), "oversized or unreadable oracle input");
    require(fclose(file) == 0, "cannot close oracle input");
    return size;
}

static ZSTD_dictContentType_e dictionary_mode(const char* mode) {
    if (!strcmp(mode, "raw")) return ZSTD_dct_rawContent;
    if (!strcmp(mode, "auto")) return ZSTD_dct_auto;
    require(!strcmp(mode, "formatted"), "unknown dictionary content mode");
    return ZSTD_dct_fullDict;
}

/* probe DICT FRAME EXPECTED MODE METHOD OUTPUT_CHUNK */
static void probe(char** args) {
    unsigned char* dictionary = malloc(CAPACITY);
    unsigned char* frame = malloc(CAPACITY);
    unsigned char* expected = malloc(CAPACITY);
    unsigned char* actual = malloc(CAPACITY);
    unsigned char scratch[1024];
    size_t dictionary_size, frame_size, expected_size, size = 0, consumed = 0;
    size_t result = 0, calls = 0;
    ZSTD_DCtx* context = ZSTD_createDCtx();
    ZSTD_dictContentType_e mode = dictionary_mode(args[3]);
    const char* method = args[4];
    require(dictionary && frame && expected && actual && context, "oracle allocation failed");
    dictionary_size = read_file(args[0], dictionary);
    frame_size = read_file(args[1], frame);
    expected_size = read_file(args[2], expected);
    if (!strcmp(method, "prepared")) {
        ZSTD_DDict* prepared = ZSTD_createDDict_advanced(dictionary, dictionary_size,
            ZSTD_dlm_byRef, mode, ZSTD_defaultCMem);
        require(prepared != NULL, "prepared dictionary loading failed");
        result = ZSTD_decompress_usingDDict(context, actual, CAPACITY, frame, frame_size, prepared);
        if (!ZSTD_isError(result)) { size = result; consumed = frame_size; }
        ZSTD_freeDDict(prepared);
    } else {
        result = ZSTD_DCtx_loadDictionary_advanced(context, dictionary, dictionary_size,
                                                  ZSTD_dlm_byRef, mode);
        require(!ZSTD_isError(result), "direct dictionary loading failed");
        if (!strcmp(method, "direct")) {
            result = ZSTD_decompressDCtx(context, actual, CAPACITY, frame, frame_size);
            if (!ZSTD_isError(result)) { size = result; consumed = frame_size; }
        } else {
            size_t chunk = (size_t)strtoul(args[5], NULL, 10);
            require(!strcmp(method, "stream"), "unknown oracle method");
            require(chunk > 0 && chunk <= sizeof(scratch), "invalid oracle output chunk");
            do {
                size_t offered = consumed < frame_size ? 1 : 0;
                ZSTD_inBuffer input = {frame + consumed, offered, 0};
                ZSTD_outBuffer output = {scratch, chunk, 0};
                result = ZSTD_decompressStream(context, &output, &input);
                consumed += input.pos;
                require(size + output.pos <= CAPACITY, "oracle output exceeds bound");
                memcpy(actual + size, scratch, output.pos);
                size += output.pos;
                require(++calls < 2000000, "oracle made no forward progress");
                require(ZSTD_isError(result) || !result || offered || output.pos,
                        "oracle requested input after source exhaustion");
            } while (!ZSTD_isError(result) && (result || consumed < frame_size));
        }
    }
    printf("{\"success\":%s,\"error\":\"%s\",\"produced\":%zu,"
           "\"consumed\":%zu,\"matches_expected\":%s,\"calls\":%zu}\n",
           ZSTD_isError(result) ? "false" : "true", ZSTD_getErrorName(result), size,
           consumed, size == expected_size && !memcmp(actual, expected, size) ? "true" : "false", calls);
    ZSTD_freeDCtx(context);
    free(actual); free(expected); free(frame); free(dictionary);
}

int main(int argc, char** argv) {
    require(ZSTD_versionNumber() == 10507, "oracle must be zstd v1.5.7");
    require(argc == 8 && !strcmp(argv[1], "probe"),
            "usage: dictionary-vectors probe DICT FRAME EXPECTED MODE METHOD OUTPUT_CHUNK");
    probe(argv + 2);
    return 0;
}
