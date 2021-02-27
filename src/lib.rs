//! # High level bindings to Wren
//!
//! [wren.io](https://wren.io/)
//!
//! ## Goals
//!
//! - high level interface; safety; not-performance
//!
//! ## Language interoperability
//!
//! - Current limitation of foreign classes only
//! - Rust can run interpret to execute Wren code
//! - Rust can call Wren function handles
//! - Wren interfaces with Rust via foreign method calls
//! - Value conversion happen during
//!
//! ## Safety
//!
//! - Wren has bugs
//! - Inner mutability (foreign value stored in RefCell)
#[macro_use]
extern crate lazy_static;

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
#[allow(clippy::redundant_static_lifetimes)]
#[doc(hidden)]
pub mod bindings;

pub mod class;
mod errors;
pub mod foreign;
pub mod handle;
mod runtime;
mod types;
pub mod value;
mod vm;

pub use errors::*;
pub use vm::*;

pub mod prelude {
    pub use crate::class::{WrenCell, WrenForeignClass};
    pub use crate::handle::WrenRef;
    pub use crate::value::{FromWren, ToWren};
    pub use crate::vm::{WrenBuilder, WrenVm};
    pub use rust_wren_derive::{foreign_error, wren_class, wren_methods};
}

/// Modules that are needed by generated code, but not meant to be part
/// part of our API.
#[doc(hidden)]
pub mod generation {
    pub use inventory;
}
