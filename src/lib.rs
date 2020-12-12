#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
#[allow(clippy::redundant_static_lifetimes)]
#[doc(hidden)]
pub mod bindings;

pub mod class;
pub mod foreign;
pub mod handle;
mod runtime;
mod types;
pub mod value;
mod vm;

pub use vm::*;

pub mod prelude {
    pub use crate::class::{WrenCell, WrenForeignClass};
    pub use crate::handle::WrenRef;
    pub use crate::value::FromWren;
    pub use crate::vm::{WrenBuilder, WrenVm};
    pub use rust_wren_derive::{wren_class, wren_methods};
}

/// Modules that are needed by generated code, but not meant to be part
/// part of our API.
#[doc(hidden)]
pub mod generation {
    pub use inventory;
}

pub trait HelloMacro {
    fn hello_macro();
}
