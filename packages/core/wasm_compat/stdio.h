#ifndef YQ_WASM_COMPAT_STDIO_H
#define YQ_WASM_COMPAT_STDIO_H
#include_next <stdio.h>
#ifdef __wasm__
FILE *fdopen(int fd, const char *mode);
#endif
#endif
