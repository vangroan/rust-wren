extern crate bindgen;

use std::io::{self, Write};
use std::{
    env,
    path::PathBuf,
    process::{Command, ExitStatus},
};

fn build_win64() {
    println!("Building Wren");

    let output = Command::new("msbuild.exe")
        .arg(r"wren\projects\vs2019\wren.vcxproj")
        .arg("/property:Configuration=Release 64bit")
        .arg("/property:Platform=x64")
        .output()
        .expect("Failed to invoke MSBuild");

    println!("status: {}", output.status);
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    if !output.status.success() {
        panic!("MSBuild Failed");
    }
}

fn generate_bindings() {
    println!("cargo:rustc-link-search=wren/lib");

    // Tell cargo to tell rustc to link the wren
    // shared library.
    println!("cargo:rustc-link-lib=wren");

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
    build_win64();
    generate_bindings();
}
