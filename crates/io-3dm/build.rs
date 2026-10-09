//! Builds openNURBS (third_party/opennurbs, git submodule) as a static library with
//! CMake, then compiles our C ABI shim against it. See docs/adr/0002-3dm-io.md.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let on_src = manifest.join("../../third_party/opennurbs");
    if !on_src.join("opennurbs_public.h").exists() {
        panic!(
            "openNURBS sources not found at {}. Run: git submodule update --init --depth 1",
            on_src.display()
        );
    }
    let target = std::env::var("TARGET").unwrap();
    let msvc = target.contains("msvc");

    // 1. openNURBS + its bundled zlib (and freetype/uuid on Linux), always optimised:
    //    a debug openNURBS makes reading large files painfully slow.
    let dst = cmake::Config::new(&on_src)
        .profile("Release")
        .build_target("opennurbsStatic")
        .build();
    let build = dst.join("build");
    for sub in [
        "",
        "Release",
        "zlib",
        "zlib/Release",
        "freetype263",
        "freetype263/Release",
        "android_uuid",
        "android_uuid/Release",
    ] {
        let p = build.join(sub);
        if p.exists() {
            println!("cargo:rustc-link-search=native={}", p.display());
        }
    }

    // 2. Our shim (C ABI). Compiled first so it comes first on the link line.
    let mut shim = cc::Build::new();
    shim.cpp(true)
        .file(manifest.join("shim/shim.cpp"))
        .include(&on_src)
        .define("ON_CMAKE_BUILD", None)
        .warnings(false);
    set_cpp_std(&mut shim, msvc);
    if msvc {
        shim.define("UNICODE", None).define("_UNICODE", None);
    }
    shim.compile("forma_3dm_shim");

    // 3. Libraries, in dependency order.
    println!("cargo:rustc-link-lib=static=opennurbsStatic");
    if !msvc && !target.contains("apple") {
        println!("cargo:rustc-link-lib=static=opennurbs_public_freetype");
    }
    println!("cargo:rustc-link-lib=static=zlib");

    // zlib calls back into openNURBS for memory (zcalloc/zcfree). Those live in an
    // object nothing else references, so build it ourselves and link it after zlib.
    let mut zmem = cc::Build::new();
    zmem.cpp(true)
        .file(on_src.join("opennurbs_zlib_memory.cpp"))
        .include(&on_src)
        .define("ON_COMPILING_OPENNURBS", None)
        .define("ON_CMAKE_BUILD", None)
        .warnings(false)
        .cargo_metadata(false);
    set_cpp_std(&mut zmem, msvc);
    if msvc {
        zmem.define("UNICODE", None).define("_UNICODE", None);
    }
    zmem.compile("forma_on_zmem");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=forma_on_zmem");

    if msvc {
        // rpcrt4: UUIDs, shlwapi: paths, gdi32/user32: fonts and resource strings,
        // advapi32: GetUserNameW (revision history).
        for lib in ["rpcrt4", "shlwapi", "gdi32", "user32", "advapi32"] {
            println!("cargo:rustc-link-lib={lib}");
        }
    } else if target.contains("apple") {
        println!("cargo:rustc-link-lib=c++");
        for fw in ["CoreGraphics", "CoreText", "Foundation"] {
            println!("cargo:rustc-link-lib=framework={fw}");
        }
    } else {
        println!("cargo:rustc-link-lib=static=android_uuid");
        println!("cargo:rustc-link-lib=stdc++");
    }

    println!("cargo:rerun-if-changed=shim/shim.cpp");
    println!("cargo:rerun-if-changed=build.rs");
    rerun_if_submodule_moves(&on_src);
}

fn set_cpp_std(b: &mut cc::Build, msvc: bool) {
    if msvc {
        b.flag("/std:c++17").flag("/bigobj");
    } else {
        b.flag("-std=c++17");
    }
}

fn rerun_if_submodule_moves(on_src: &Path) {
    let head = on_src.join(".git");
    if head.exists() {
        println!("cargo:rerun-if-changed={}", head.display());
    }
}
