/* Test-only calls to the unchanged unsalted ZSTD_hashPtr (width eight).
 * facebook/zstd v1.5.7, f8745da6ff1ad1e7bab384bd1f9d742439278e99.
 * No parser or codec implementation, dynamic allocation or pointer casting.
 */
#define _POSIX_C_SOURCE 200809L
#define ZSTD_STATIC_LINKING_ONLY
#include "compress/zstd_compress_internal.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

static void require(int valid, const char *message) {
  if (!valid) {
    fprintf(stderr, "double-fast hash oracle: %s\n", message);
    exit(1);
  }
}

int main(int argc, char **argv) {
  unsigned char patterns[512];
  _Alignas(8) unsigned char input[16];
  unsigned log, shift, pattern, byte;
  struct stat status;
  FILE *file;
  require(argc == 2, "usage: double-fast-hashes patterns.bin");
  require(stat(argv[1], &status) == 0 && S_ISREG(status.st_mode) &&
              status.st_size == sizeof(patterns),
          "exact regular 512-byte pattern file");
  file = fopen(argv[1], "rb");
  require(file != NULL, "open patterns");
  require(fstat(fileno(file), &status) == 0 && S_ISREG(status.st_mode) &&
              status.st_size == sizeof(patterns),
          "opened pattern file extent");
  require(fread(patterns, 1, sizeof(patterns), file) == sizeof(patterns),
          "read all patterns");
  require(fgetc(file) == EOF && !ferror(file), "exact EOF");
  require(fclose(file) == 0, "close patterns");
  puts("log\tshift\tpattern\tbytes\tbucket");
  for (log = 6; log <= 9; ++log) {
    for (shift = 0; shift <= 1; ++shift) {
      for (pattern = 0; pattern < 64; ++pattern) {
        size_t bucket;
        /* Both eight-byte reads fit this initialized sixteen-byte stack
         * array. _Alignas(8) makes shift0 aligned and shift1 unaligned. */
        memset(input, 0xa5, sizeof(input));
        memcpy(input + shift, patterns + 8 * pattern, 8);
        bucket = ZSTD_hashPtr(input + shift, log, 8);
        require(bucket < ((size_t)1 << log), "bucket extent");
        printf("%u\t%u\t%u\t", log, shift, pattern);
        for (byte = 0; byte < 8; ++byte)
          printf("%02x", input[shift + byte]);
        printf("\t%zu\n", bucket);
      }
    }
  }
  require(fflush(stdout) == 0 && !ferror(stdout), "stdout failure");
  return 0;
}
