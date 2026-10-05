/* src/csrc/re2_shim.h
 *
 * C shim over RE2's C++ API, for src/external/re2.rs. RE2 has no C API
 * of its own, so this exposes the minimum surface that wrapper needs.
 * See docs/EXTERNAL_ENGINES.md. */
#ifndef REGEX_ENGINE_RE2_SHIM_H
#define REGEX_ENGINE_RE2_SHIM_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct Re2Handle Re2Handle;

/* Compile `pattern` (not necessarily NUL-terminated). `posix` selects
 * RE2::Options::posix_syntax + longest_match; the shim also enables
 * perl_classes/word_boundary so `\s`/`\d`/`\w`/`\b` still parse.
 * `dot_all` sets RE2::Options::dot_nl, so `.` also matches `\n`. */
Re2Handle *re2_new(const char *pattern, size_t pattern_len, int posix, int case_insensitive, int dot_all, char **error_out);

void re2_free(Re2Handle *handle);
void re2_free_error(char *error);

/* Unanchored substring search (RE2::PartialMatch). */
int re2_partial_match(const Re2Handle *handle, const char *text, size_t text_len);

/* Anchored, whole-string match (RE2::FullMatch). */
int re2_full_match(const Re2Handle *handle, const char *text, size_t text_len);

/* Unanchored search, also reporting the overall match's byte offsets.
 * The two boolean calls above cannot distinguish leftmost-first from
 * POSIX leftmost-longest; only the span can. */
int re2_partial_match_span(const Re2Handle *handle, const char *text, size_t text_len,
                            size_t *match_start, size_t *match_end);

#ifdef __cplusplus
}
#endif

#endif