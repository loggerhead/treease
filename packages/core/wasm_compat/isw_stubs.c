// Stub implementations of wide-char classification functions for wasm32 targets.
// WASI libc declares these but doesn't provide implementations, causing unresolved
// "env" imports. Each function follows the C standard definition for the ASCII subset.
#include <wctype.h>

int iswalpha(wint_t c) {
  return (c >= L'a' && c <= L'z') || (c >= L'A' && c <= L'Z');
}

int iswspace(wint_t c) {
  return c == L' ' || c == L'\t' || c == L'\n' || c == L'\r' || c == L'\v' || c == L'\f';
}

int iswxdigit(wint_t c) {
  return (c >= L'0' && c <= L'9') || (c >= L'a' && c <= L'f') || (c >= L'A' && c <= L'F');
}
