extern crate bindgen;
extern crate cc;

use glob::glob;
use std::{env, path::PathBuf, process::Command};
use std::{
    fs,
    io::{self, Write},
};

#[allow(dead_code)]
#[deprecated]
fn build_win64() {
    println!("Building Wren");

    let profile = match env::var("PROFILE").expect("PROFILE not set").as_str() {
        "release" => "Release",
        "debug" => "Debug",
        s => {
            panic!("Unsupported profile {}", s);
        }
    };

    let platform = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH not set");

    let arch_conf = match platform.as_str() {
        "x86" => "32bit",
        "x86_64" => "64bit",
        _ => {
            panic!("Unsupported architecture {}", platform);
        }
    };

    let arch_plat = match platform.as_str() {
        "x86" => "Win32",
        "x86_64" => "x64",
        _ => {
            panic!("Unsupported architecture {}", platform);
        }
    };

    let output = Command::new("msbuild.exe")
        .arg(r"wren\projects\vs2019\wren.vcxproj")
        .arg(format!("/property:Configuration={} {}", profile, arch_conf))
        .arg(format!("/property:Platform={}", arch_plat))
        .output()
        .expect("Failed to invoke MSBuild");

    println!("status: {}", output.status);
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    if !output.status.success() {
        panic!("MSBuild Failed");
    }

    let in_path = PathBuf::from(r"wren/lib");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let filename = if env::var("PROFILE").expect("PROFILE not set") == "debug" {
        "wren_d.lib"
    } else {
        "wren.lib"
    };
    fs::copy(in_path.join(filename), out_path.join(filename)).expect("Wren lib copy failed");
}

fn build_cc() {
    let output = if env::var("PROFILE").expect("PROFILE not set") == "debug" {
        "wren_d"
    } else {
        "wren"
    };

    cc::Build::new()
        .include("wren/src/include")
        .include("wren/src/optional")
        .include("wren/src/vm")
        .files(
            glob("wren/src/vm/*.c")
                .expect("failed to read glob pattern")
                .filter_map(Result::ok),
        )
        .files(
            glob("wren/src/optional/*.c")
                .expect("failed to read glob pattern")
                .filter_map(Result::ok),
        )
        .compile(output);
}

fn generate_bindings() {
    let profile = env::var("PROFILE").expect("PROFILE not set");

    // println!("cargo:rustc-link-search=wren/lib");
    println!("cargo:rustc-link-search={}", env::var("OUT_DIR").unwrap());

    // Tell cargo to tell rustc to link the wren
    // shared library.
    let lib_filename = if profile.as_str() == "debug" { "wren_d" } else { "wren" };
    println!("cargo:rustc-link-lib={}", lib_filename);

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // On most platforms size_t and usize are the same.
        // Otherwise generated bindings would be either x86 or x64 specific.
        .size_t_is_usize(true)
        // On Linux these functions return "long double" which
        // gets converted to u128. The ABI for i128 and u128 are
        // undefined and result in clippy warnings.
        .blocklist_function("strtold")
        .blocklist_function("wcstold")
        .blocklist_function("qecvt")
        .blocklist_function("qfcvt")
        .blocklist_function("qgcvt")
        .blocklist_function("ecvt_r")
        .blocklist_function("qecvt_r")
        .blocklist_function("qfcvt_r")
        .blocklist_item("_Float64x")
        .blocklist_item("__HAVE_FLOAT64X")
        .blocklist_item("__HAVE_FLOAT64X_LONG_DOUBLE")
        .blocklist_item("__HAVE_DISTINCT_FLOAT64X")
        .blocklist_item("__HAVE_DISTINCT_FLOAT128X")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // Write the bindings to the src/bindings.rs file.
    bindings
        .write_to_file("src/bindings.rs")
        .expect("Couldn't write bindings!");
}

fn main() {
    build_cc();
    generate_bindings();
}
