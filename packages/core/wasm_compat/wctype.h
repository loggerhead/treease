#ifndef YQ_WASM_COMPAT_WCTYPE_H
#define YQ_WASM_COMPAT_WCTYPE_H
// Override wctype.h for wasm32 targets: define wide-char predicates as macros
// to avoid unresolved imports (WASI libc declares these but doesn't provide
// implementations).
#define iswalpha(c) ((c) >= L'a' && (c) <= L'z' || (c) >= L'A' && (c) <= L'Z')
#define iswdigit(c) ((c) >= L'0' && (c) <= L'9')
#define iswalnum(c) (iswalpha(c) || iswdigit(c))
#define iswxdigit(c)                                                           \
  ((c) >= L'0' && (c) <= L'9' || (c) >= L'a' && (c) <= L'f' || (c) >= L'A' && \
   (c) <= L'F')
#define iswspace(c)                                                            \
  ((c) == L' ' || (c) == L'\t' || (c) == L'\n' || (c) == L'\r' ||             \
   (c) == L'\v' || (c) == L'\f')
#define iswblank(c) ((c) == L' ' || (c) == L'\t')
#define iswprint(c) ((c) >= L' ' && (c) <= L'~')
#endif