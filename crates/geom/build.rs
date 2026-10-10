//! With feature `occt`: build OpenCascade (static, via `occt-sys`) unless
//! `FORMA_OCCT_DIR` already holds a build, then compile our C ABI shim
//! (`occt/shim.cpp`) against it. See docs/adr/0004-occt-crates.md.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(feature = "occt")]
    occt::build();
}

#[cfg(feature = "occt")]
mod occt {
    use std::path::{Path, PathBuf};

    /// OCCT toolkits we use, in link order (dependents before dependencies).
    const LIBS: &[&str] = &[
        "TKOffset",
        "TKFillet",
        "TKBool",
        "TKBO",
        "TKMesh",
        "TKShHealing",
        "TKPrim",
        "TKTopAlgo",
        "TKGeomAlgo",
        "TKBRep",
        "TKGeomBase",
        "TKG3d",
        "TKG2d",
        "TKMath",
        "TKernel",
    ];

    fn is_install(dir: &Path) -> bool {
        dir.join("include/Standard.hxx").exists() && dir.join("lib").is_dir()
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("create OCCT cache dir");
        for entry in std::fs::read_dir(from).expect("read OCCT dir") {
            let entry = entry.expect("dir entry");
            let target = to.join(entry.file_name());
            if entry.file_type().expect("file type").is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).expect("copy OCCT file");
            }
        }
    }

    /// Where OCCT's `include/` and `lib/` are, building it when needed.
    fn occt_dir() -> PathBuf {
        println!("cargo:rerun-if-env-changed=FORMA_OCCT_DIR");
        match std::env::var_os("FORMA_OCCT_DIR").map(PathBuf::from) {
            Some(dir) if is_install(&dir) => dir,
            Some(dir) => {
                // Build once, then keep only include/ and lib/ in the given directory
                // (CI caches it; the CMake build tree stays in target/).
                occt_sys::build_occt();
                let built = occt_sys::occt_path();
                copy_dir(&built.join("include"), &dir.join("include"));
                copy_dir(&built.join("lib"), &dir.join("lib"));
                dir
            }
            None => {
                occt_sys::build_occt();
                occt_sys::occt_path()
            }
        }
    }

    pub fn build() {
        let target = std::env::var("TARGET").expect("TARGET");
        let msvc = target.contains("msvc");
        let windows = target.contains("windows");
        let dir = occt_dir();

        let mut shim = cc::Build::new();
        shim.cpp(true)
            .file("occt/shim.cpp")
            .include(dir.join("include"))
            .define("_USE_MATH_DEFINES", None)
            .warnings(false);
        if msvc {
            shim.flag("/std:c++17")
                .flag("/bigobj")
                .define("OCCT_STATIC_BUILD", None);
        } else {
            shim.flag("-std=c++17");
            if windows {
                shim.define("OCCT_STATIC_BUILD", None);
            }
        }
        shim.compile("forma_occt_shim");

        println!(
            "cargo:rustc-link-search=native={}",
            dir.join("lib").display()
        );
        for lib in LIBS {
            println!("cargo:rustc-link-lib=static={lib}");
        }
        if windows {
            for lib in ["advapi32", "gdi32", "user32", "ws2_32", "psapi", "shell32"] {
                println!("cargo:rustc-link-lib={lib}");
            }
        } else if target.contains("apple") {
            println!("cargo:rustc-link-lib=c++");
        } else {
            for lib in ["stdc++", "pthread", "dl", "m"] {
                println!("cargo:rustc-link-lib={lib}");
            }
        }
        println!("cargo:rerun-if-changed=occt/shim.cpp");
    }
}
