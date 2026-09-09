#include "re2_shim.h"

#include <re2/re2.h>

#include <cstdlib>
#include <cstring>
#include <new>
#include <string>

struct Re2Handle {
    RE2 re;
    Re2Handle(const re2::StringPiece &pattern, const RE2::Options &opts) : re(pattern, opts) {}
};

static char *dup_error(const std::string &msg) {
    char *out = static_cast<char *>(std::malloc(msg.size() + 1));
    if (out != nullptr) {
        std::memcpy(out, msg.c_str(), msg.size() + 1);
    }
    return out;
}

Re2Handle *re2_new(const char *pattern, size_t pattern_len, int posix, int case_insensitive, char **error_out) {
    RE2::Options opts;
    opts.set_log_errors(false); /* callers get the error via error_out */
    if (posix) {
        opts.set_posix_syntax(true);
        opts.set_longest_match(true);
        /* Keep the same escape dialect as non-POSIX mode, 
         * so the two only differ in disambiguation policy, not what syntax they accept. 
         */
        opts.set_perl_classes(true);
        opts.set_word_boundary(true);
    }
    if (case_insensitive) {
        opts.set_case_sensitive(false);
    }

    re2::StringPiece p(pattern, pattern_len);
    Re2Handle *handle = new (std::nothrow) Re2Handle(p, opts);
    if (handle == nullptr) {
        if (error_out != nullptr) {
            *error_out = dup_error("out of memory");
        }
        return nullptr;
    }
    if (!handle->re.ok()) {
        if (error_out != nullptr) {
            *error_out = dup_error(handle->re.error());
        }
        delete handle;
        return nullptr;
    }
    return handle;
}

void re2_free(Re2Handle *handle) {
    delete handle;
}

void re2_free_error(char *error) {
    std::free(error);
}

int re2_partial_match(const Re2Handle *handle, const char *text, size_t text_len) {
    re2::StringPiece t(text, text_len);
    return RE2::PartialMatch(t, handle->re) ? 1 : 0;
}

int re2_full_match(const Re2Handle *handle, const char *text, size_t text_len) {
    re2::StringPiece t(text, text_len);
    return RE2::FullMatch(t, handle->re) ? 1 : 0;
}

int re2_partial_match_span(const Re2Handle *handle, const char *text, size_t text_len,
                            size_t *match_start, size_t *match_end) {
    re2::StringPiece t(text, text_len);
    re2::StringPiece submatch;
    if (!handle->re.Match(t, 0, text_len, RE2::UNANCHORED, &submatch, 1)) {
        return 0;
    }
    *match_start = static_cast<size_t>(submatch.data() - text);
    *match_end = *match_start + submatch.size();
    return 1;
}
