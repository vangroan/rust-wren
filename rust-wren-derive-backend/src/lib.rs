//! Functionality backing the procedural macros in the `rust-wren-derive` crate.
//!
//! A `proc_macro` crate cannot export anything that's not a procedural macro, requiring
//! any public functions or structs to live in a seperate crate.
mod class;
mod method;

pub use class::{gen_from_wren_impl, gen_to_wren_impl, WrenClassArgs};
pub use method::build_wren_methods;
