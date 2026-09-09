/* C shim over RE2's C++ API. 
 * RE2 has no official C API of its own, so this exposes the minimum surface `src/external/re2.rs` needs: 
 * compile a pattern (optionally in POSIX leftmost-longest mode) 
 * And run an unanchored or anchored match against it. See docs/EXTERNAL_ENGINES.md. */
#ifndef REGEX_ENGINE_RE2_SHIM_H
#define REGEX_ENGINE_RE2_SHIM_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct Re2Handle Re2Handle;

/* Compiles `pattern` (`pattern_len` bytes, not necessarily NUL-terminated)
 * `posix` selects RE2::Options::posix_syntax + longest_match (POSIX leftmost-longest) 
 * Selects over RE2's default leftmost-first (Perl-like) mode when set
 * Also enables perl_classes/word_boundary so `\s`/`\d`/`\w`/ `\b` still parse 
 * (posix_syntax otherwise restricts to bare POSIX ERE syntax) */
Re2Handle *re2_new(const char *pattern, size_t pattern_len, int posix, int case_insensitive, char **error_out);

void re2_free(Re2Handle *handle);
void re2_free_error(char *error);

/* Unanchored substring search (RE2::PartialMatch)*/
int re2_partial_match(const Re2Handle *handle, const char *text, size_t text_len);

/* Anchored, whole-string match (RE2::FullMatch)*/
int re2_full_match(const Re2Handle *handle, const char *text, size_t text_len);

/* Unanchored search that also reports the byte offsets of the overall match
 * (relative to the start of `text`) on success. 
 * Whole-string membership (re2_partial_match/re2_full_match) can never tell leftmost-first and
 * POSIX leftmost-longest apart 
 * Both modes accept the same language, so they always agree on yes/no. 
 * Only the match span differs between them*/
int re2_partial_match_span(const Re2Handle *handle, const char *text, size_t text_len,
                            size_t *match_start, size_t *match_end);

#ifdef __cplusplus
}
#endif

#endif
