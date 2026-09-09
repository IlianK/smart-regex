use std::env;

fn main() {
    println!("cargo:rerun-if-changed=csrc/re2_shim.cpp");
    println!("cargo:rerun-if-changed=csrc/re2_shim.h");

    if env::var("CARGO_FEATURE_EXTERNAL_ENGINES").is_err() {
        return;
    }

    let re2 = pkg_config::probe_library("re2").unwrap_or_else(|e| {
        panic!(
            "\n\nexternal-engines requires the RE2 C++ library, found via \
             pkg-config's `re2` package -- pkg-config lookup failed:\n{}\n\n\
             Install the RE2 development package first (e.g. `apt install \
             libre2-dev` on Debian/Ubuntu, `brew install re2` on macOS), then \
             rebuild. See docs/EXTERNAL_ENGINES.md.\n",
            e
        )
    });

    let mut build = cc::Build::new();
    build.cpp(true).std("c++17").file("csrc/re2_shim.cpp");
    for inc in &re2.include_paths {
        build.include(inc);
    }
    build.compile("re2_shim");
}
