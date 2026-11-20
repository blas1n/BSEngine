use std::env;
use std::path::PathBuf;

/// All 22 core library source files.
const SOURCES: &[&str] = &[
    "cpp/astcenc_averages_and_directions.cpp",
    "cpp/astcenc_block_sizes.cpp",
    "cpp/astcenc_color_quantize.cpp",
    "cpp/astcenc_color_unquantize.cpp",
    "cpp/astcenc_compress_symbolic.cpp",
    "cpp/astcenc_compute_variance.cpp",
    "cpp/astcenc_decompress_symbolic.cpp",
    "cpp/astcenc_diagnostic_trace.cpp",
    "cpp/astcenc_entry.cpp",
    "cpp/astcenc_find_best_partitioning.cpp",
    "cpp/astcenc_ideal_endpoints_and_weights.cpp",
    "cpp/astcenc_image.cpp",
    "cpp/astcenc_integer_sequence.cpp",
    "cpp/astcenc_mathlib.cpp",
    "cpp/astcenc_mathlib_softfloat.cpp",
    "cpp/astcenc_partition_tables.cpp",
    "cpp/astcenc_percentile_tables.cpp",
    "cpp/astcenc_pick_best_endpoint_format.cpp",
    "cpp/astcenc_quantization.cpp",
    "cpp/astcenc_symbolic_physical.cpp",
    "cpp/astcenc_weight_align.cpp",
    "cpp/astcenc_weight_quant_xfer_tables.cpp",
];

/// Public API functions that need per-ISA symbol renaming.
const PUBLIC_FUNCTIONS: &[&str] = &[
    "astcenc_config_init",
    "astcenc_context_alloc",
    "astcenc_context_free",
    "astcenc_compress_image",
    "astcenc_compress_reset",
    "astcenc_compress_cancel",
    "astcenc_decompress_image",
    "astcenc_decompress_reset",
    "astcenc_get_block_info",
    "astcenc_get_error_string",
];

struct IsaVariant {
    name: &'static str,
    lib_name: &'static str,
    defines: &'static [(&'static str, &'static str)],
    /// Compiler flags for GCC/Clang only.
    gnu_flags: &'static [&'static str],
    /// Compiler flags for MSVC only.
    msvc_flags: &'static [&'static str],
}

fn main() {
    println!("cargo:rerun-if-changed=cpp");

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let is_msvc = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default() == "msvc";

    let variants: Vec<IsaVariant> = match target_arch.as_str() {
        "x86_64" => vec![
            IsaVariant {
                name: "sse2",
                lib_name: "astcenc_sse2",
                defines: &[
                    ("ASTCENC_SSE", "20"),
                    ("ASTCENC_AVX", "0"),
                    ("ASTCENC_NEON", "0"),
                    ("ASTCENC_SVE", "0"),
                    ("ASTCENC_POPCNT", "0"),
                    ("ASTCENC_F16C", "0"),
                    ("ASTCENC_X86_GATHERS", "0"),
                ],
                gnu_flags: &["-msse2"],
                msvc_flags: &[],
            },
            IsaVariant {
                name: "sse41",
                lib_name: "astcenc_sse41",
                defines: &[
                    ("ASTCENC_SSE", "41"),
                    ("ASTCENC_AVX", "0"),
                    ("ASTCENC_NEON", "0"),
                    ("ASTCENC_SVE", "0"),
                    ("ASTCENC_POPCNT", "1"),
                    ("ASTCENC_F16C", "0"),
                    ("ASTCENC_X86_GATHERS", "0"),
                ],
                gnu_flags: &["-msse4.1", "-mpopcnt"],
                msvc_flags: &[],
            },
            IsaVariant {
                name: "avx2",
                lib_name: "astcenc_avx2",
                defines: &[
                    ("ASTCENC_SSE", "41"),
                    ("ASTCENC_AVX", "2"),
                    ("ASTCENC_NEON", "0"),
                    ("ASTCENC_SVE", "0"),
                    ("ASTCENC_POPCNT", "1"),
                    ("ASTCENC_F16C", "1"),
                    ("ASTCENC_X86_GATHERS", "0"),
                ],
                gnu_flags: &["-mavx2", "-mpopcnt", "-mf16c"],
                msvc_flags: &["/arch:AVX2"],
            },
        ],
        "aarch64" => vec![IsaVariant {
            name: "neon",
            lib_name: "astcenc_neon",
            defines: &[
                ("ASTCENC_NEON", "1"),
                ("ASTCENC_SVE", "0"),
                ("ASTCENC_SSE", "0"),
                ("ASTCENC_AVX", "0"),
                ("ASTCENC_POPCNT", "0"),
                ("ASTCENC_F16C", "0"),
            ],
            gnu_flags: &[],
            msvc_flags: &[],
        }],
        other => panic!("unsupported target architecture: {other}"),
    };

    for variant in &variants {
        build_variant(variant, is_msvc);
    }
}

fn build_variant(variant: &IsaVariant, is_msvc: bool) {
    let mut build = cc::Build::new();

    // Use a per-variant output directory to avoid object file collisions
    // between ISA variants compiled from the same source files.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let variant_dir = out_dir.join(variant.name);
    build.out_dir(&variant_dir);

    build.cpp(true).std("c++14").include("cpp").warnings(false);

    // ISA-specific defines.
    for &(key, value) in variant.defines {
        build.define(key, value);
    }

    // Performance: allow FP contraction.
    build.define("ASTCENC_NO_INVARIANCE", "1");

    // Force extern "C" linkage on public API functions so Rust can call them.
    // This also adds dllexport/visibility attributes which are harmless for static libs.
    build.define("ASTCENC_DYNAMIC_LIBRARY", None);

    // Symbol renaming: prefix public API functions with ISA name.
    for &func in PUBLIC_FUNCTIONS {
        let renamed = format!(
            "astcenc_{name}_{bare}",
            name = variant.name,
            bare = &func["astcenc_".len()..]
        );
        build.define(func, renamed.as_str());
    }

    // ISA-specific compiler flags.
    if is_msvc {
        for &flag in variant.msvc_flags {
            build.flag(flag);
        }
        // FP contraction for non-invariant builds.
        build.flag("/fp:precise");
    } else {
        for &flag in variant.gnu_flags {
            build.flag(flag);
        }
        build.flag("-ffp-contract=fast");
    }

    // When multiple ISA variants are linked into the same binary, internal C++
    // functions (anything not in PUBLIC_FUNCTIONS) would collide — the linker
    // picks one variant's definition for all callers, causing ABI mismatches
    // (e.g. AVX2 code calling SSE2-compiled helpers with different struct sizes).
    //
    // Fix: wrap each source file in a per-variant C++ namespace. Internal C++
    // symbols get distinct mangled names per variant. The `extern "C"` public
    // API functions are unaffected (C linkage ignores namespaces).
    //
    // Standard library headers must be included *before* the namespace opens
    // because `using ::uint16_t` etc. inside `<cstdint>` would fail.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cpp_dir = manifest_dir.join("cpp");
    std::fs::create_dir_all(&variant_dir).expect("failed to create variant output dir");

    // All standard/system headers used anywhere in the astcenc source tree.
    // Pre-including them before the namespace ensures they resolve in the
    // global namespace as the standard requires.
    let std_headers = "\
        #include <algorithm>\n\
        #include <array>\n\
        #include <atomic>\n\
        #include <cassert>\n\
        #include <cfloat>\n\
        #include <cmath>\n\
        #include <condition_variable>\n\
        #include <cstdarg>\n\
        #include <cstddef>\n\
        #include <cstdint>\n\
        #include <cstdio>\n\
        #include <cstdlib>\n\
        #include <cstring>\n\
        #include <fstream>\n\
        #include <functional>\n\
        #include <iostream>\n\
        #include <limits>\n\
        #include <mutex>\n\
        #include <new>\n\
        #include <string>\n\
        #include <type_traits>\n\
        #include <utility>\n\
        #include <vector>\n\
        #include <assert.h>\n\
        #include <stdio.h>\n\
    ";

    for &src in SOURCES {
        let file_name = std::path::Path::new(src)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let abs_src = cpp_dir.join(file_name);
        let wrapper_path = variant_dir.join(file_name);

        let wrapper = format!(
            "{std_headers}\
             namespace astcenc_{ns} {{\n\
             using ::abs;\n\
             using ::memcpy;\n\
             using ::memset;\n\
             #include \"{src}\"\n\
             }} // namespace astcenc_{ns}\n",
            std_headers = std_headers,
            ns = variant.name,
            src = abs_src.to_str().unwrap().replace('\\', "/"),
        );

        // Only rewrite if content changed to avoid unnecessary rebuilds.
        let needs_write = std::fs::read_to_string(&wrapper_path)
            .map(|existing| existing != wrapper)
            .unwrap_or(true);
        if needs_write {
            std::fs::write(&wrapper_path, &wrapper).expect("failed to write wrapper");
        }

        build.file(&wrapper_path);
    }

    build.compile(variant.lib_name);
}
