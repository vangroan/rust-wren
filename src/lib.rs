// #![allow(non_upper_case_globals)]
// #![allow(non_camel_case_types)]
// #![allow(non_snake_case)]

// include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
#[allow(clippy::redundant_static_lifetimes)]
mod bindings;

mod runtime;
mod vm;

pub use vm::*;
