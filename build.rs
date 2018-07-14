extern crate bindgen;
extern crate cc;

use std::env;
use std::path::PathBuf;
use std::fs::*;

fn main() {
    // TODO create static library
    println!("cargo:rustc-link-lib=static=mastik");
    println!("cargo:rustc-link-lib=static=dwarf");
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-lib=elf");
    // println!("cargo:rustc-link-lib=dwarf");
    println!("cargo:rustc-link-lib=bfd");
    println!("cargo:rustc-link-search=native=../../attack/cache/Mastik-0.02-AyeAyeCapn/src");
    // println!("cargo:libdir=xxx");
    // println!("cargo:warning=dynamic library");


    let bindings = bindgen::Builder::default()
        .rustfmt_bindings(true)
        // .whitelist_recursively(true)
        .header("csrc/mastik.h")
        .header("../../attack/cache/Mastik-0.02-AyeAyeCapn/src/symbol.h")
        .header("../../attack/cache/Mastik-0.02-AyeAyeCapn/src/util.h")
        // .whitelist_type(r"msr_batch_.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    if !out_path.exists() {
        create_dir_all(out_path.clone()).expect("faild to create bindgen target directory");
    }

    assert!(out_path.is_dir());

    let out_file_path = out_path.join("bindings.rs");
    bindings
        .write_to_file(&out_file_path)
        .expect("Couldn't write bindings!");

    println!("cargo:warning={}", out_file_path.to_str().unwrap());

//     cc::Build::new()
//         .file("csrc/mastik.c")
//         .define("_GNU_SOURCE", None)
//         .include("csrc/")
//         .flag("-std=gnu99")
//         .flag("-O3")
//         .flag("-march=native")
//         .flag("-mtune=native")
//         .flag("-fno-unroll-loops")
//         .flag("-g0")
// //        .flag("-pthread")
//         .shared_flag(true)
//         .static_flag(true)
//         .compile("rdpmc");

}
