//! Foreign class wrapper for storing Rust values in Wren.
//!
//! This module re-exports some [`std::cell`](https://doc.rust-lang.org/std/cell/) types as part of its interface.
//!
//! - [`Ref`](https://doc.rust-lang.org/std/cell/struct.Ref.html)
//! - [`RefMut`](https://doc.rust-lang.org/std/cell/struct.RefMut.html)
//! - [`BorrowError`](https://doc.rust-lang.org/std/cell/struct.BorrowError.html)
//! - [`BorrowMutError`](https://doc.rust-lang.org/std/cell/struct.BorrowMutError.html)
//!
//! # Examples
//!
//! The preferred method of implementing a type as a foreign class is via the attribute macros
//! `wren_class` and `wren_method`.
//!
//! ```
//! use rust_wren::prelude::*;
//!
//! #[wren_class(name=Sprite)]
//! struct WrenSprite {
//!     position: Pos,
//! }
//!
//! // Currently there is a limitation that a `wren_class` definition must
//! // have an accompanying `wren_methods` definitions, and at least a `construct` method.
//! #[wren_methods]
//! impl WrenSprite {
//!     #[construct]
//!     fn new(position: &WrenCell<Pos>) -> Self {
//!         Self {
//!             position: position.borrow().clone(),
//!         }
//!     }
//!
//!     fn get(&self) -> Pos {
//!         self.position.clone()
//!     }
//!
//!     // Note we can only borrow the `WrenCell` from the VM.
//!     fn set(&mut self, pos: &WrenCell<Pos>) {
//!         self.position = pos.borrow().clone();
//!     }
//! }
//!
//! #[wren_class]
//! #[derive(Clone)]
//! struct Pos(f64, f64);
//!
//! #[wren_methods]
//! impl Pos {
//!     #[construct]
//!     fn new(x: f64, y: f64) -> Self {
//!         Self(x, y)
//!     }
//!
//!     fn x(&self) -> f64 {
//!         self.0
//!     }
//!
//!     fn y(&self) -> f64 {
//!         self.1
//!     }
//! }
//!
//! // When creating a new VM, register the class in a module.
//! let mut vm = WrenBuilder::new()
//!     .with_module("game", |module| {
//!         module.register::<WrenSprite>();
//!         module.register::<Pos>();
//!     })
//!     .build();
//!
//! // The class needs to be declared on the Wren side.
//! // This is done by interpreting some Wren code in the same module.
//! vm.interpret(
//!     "game",
//!     r#"
//!  foreign class Sprite {
//!      construct new(position) {}
//!      foreign get()
//!      foreign set(position)
//!  }
//!
//!  foreign class Pos {
//!      construct new(x, y) {}
//!      foreign x()
//!      foreign y()
//!  }
//!  "#).expect("Interpret failed");
//!
//! // It can now be instantiated from within Wren.
//! vm.interpret(
//!     "game",
//!     r#"
//! var sprite = Sprite.new(Pos.new(7, 11))
//! var pos = sprite.get()
//! sprite.set(Pos.new(2, 4))
//! "#).expect("Interpret failed");
//! ```
use crate::{ModuleBuilder, WrenError, WrenResult};
pub use std::cell::{Ref, RefMut};
use std::{any::TypeId, cell::RefCell, fmt, os::raw::c_void};

/// Trait for any type to be registered as a foreign class.
pub trait WrenForeignClass {
    const NAME: &'static str;

    fn register(bindings: &mut ModuleBuilder);
}

/// Wrapper for foreign class values stored in Wren.
///
/// Keeps Rust type information at runtime so foreign values can be
/// safely type checked before casting from void pointers.
///
/// Foreign method calls from Wren are dynamically typed, and can reach Rust
/// with the incorrect types. These values are simply pointers, with no additional
/// meta data from Wren as to what type they are. To ensure a safe foreign
/// interface Rust must check function arguments coming from Wren.
///
/// The inner value is kept in a `RefCell` for
/// runtime borrow checking. Since foreign values live in Wren and
/// cross the border as a raw pointer, no compile-time borrow
/// checking can occur. This also means that `WrenCell` has inner
/// mutability, and a mutable borrow can be retrieved from an immutable
/// reference.
///
/// For primitive types, those that implement [`FromWren`](../value/struct.FromWren.html)
/// but not [`WrenForeignClass`](struct.WrenForeignClass.html), Wren provides functions for
/// checking the type of slots.
///
/// # Errors
///
/// The usual borrow errors from `RefCell` apply when
/// attempting an invalid borrow.
///
/// # Safety
///
/// The type checking relies on the C representation having the first
/// struct field at exactly the same memory address as the struct itself.
/// This is guaranteed in the C99 standard.
///
/// To test for type equality, a given void pointer can be cast to a
/// `std::any::TypeId` and compared to the desired Rust type (See the
/// implementation of [`WrenCell::is_type()`](#method.is_type)). If the test passes
/// the same pointer can be safely cast to the appropriate `WrenCell` type.
#[repr(C)]
pub struct WrenCell<T: 'static> {
    /// Important: Keep type_id as first field.
    type_id: TypeId,
    cell: RefCell<T>,
}

impl<T> WrenCell<T>
where
    T: 'static,
{
    /// Create a new `WrenCell` by wrapping the given value.
    pub fn new(inner: T) -> Self {
        WrenCell {
            type_id: TypeId::of::<T>(),
            cell: RefCell::new(inner),
        }
    }

    /// Create a new `WrenCell` from the given `RefCell`.
    pub fn from_cell(cell: RefCell<T>) -> Self {
        WrenCell {
            type_id: TypeId::of::<T>(),
            cell,
        }
    }

    /// Create a new `WrenCell` by casting the given raw pointer.
    ///
    /// The pointer must be to a `WrenCell<T>`.
    ///
    /// # Safety
    ///
    /// The given value is cast to a `WrenCell<T>`, and is accepted if it's type tag matches.
    /// A pointer to data of another type - which passes the type check regardless - will return a
    /// `Some` containing a `WrenCell` cast from bad memory.
    #[inline]
    pub unsafe fn from_ptr<'a>(maybe_cell: *const c_void) -> WrenResult<&'a Self> {
        if !Self::is_type(maybe_cell) {
            Err(WrenError::ForeignType)
        } else {
            (maybe_cell as *const Self).as_ref().ok_or(WrenError::NullPtr)
        }
    }

    /// Create a new `WrenCell` by casting the given raw pointer.
    ///
    /// The pointer must be to a `WrenCell<T>`.
    ///
    /// # Safety
    ///
    /// The given value is cast to a `WrenCell<T>`, and is accepted if it's type tag matches.
    /// A pointer to data of another type - which passes the type check regardless - will return a
    /// `Some` containing a `WrenCell` cast from bad memory.
    #[inline]
    pub unsafe fn from_ptr_mut<'a>(maybe_cell: *mut c_void) -> WrenResult<&'a mut Self> {
        if !Self::is_type(maybe_cell) {
            Err(WrenError::ForeignType)
        } else {
            (maybe_cell as *mut Self).as_mut().ok_or(WrenError::NullPtr)
        }
    }

    #[inline]
    pub fn borrow(&self) -> Ref<'_, T> {
        self.cell.borrow()
    }

    #[inline]
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.cell.borrow_mut()
    }

    #[inline]
    pub fn try_borrow(&self) -> WrenResult<Ref<'_, T>> {
        self.cell.try_borrow().map_err(|_| WrenError::BorrowError)
    }

    #[inline]
    pub fn try_borrow_mut(&self) -> WrenResult<RefMut<'_, T>> {
        self.cell.try_borrow_mut().map_err(|_| WrenError::BorrowMutError)
    }

    /// Given a pointer to a `WrenCell`, check if the contents type matches the type
    /// of this cell's contents.
    ///
    /// # Safety
    ///
    /// A cast is performed on the given pointer. If it points to some other value, or points to
    /// junk, then the behaviour is undefined.
    ///
    /// A null pointer always returns false.
    #[inline]
    pub fn is_type(maybe_cell: *const c_void) -> bool {
        if maybe_cell.is_null() {
            false
        } else {
            unsafe {
                // This relies on the C99 guarantee that the first
                // field of a struct has 0 offset, and is at the same
                // memory location as the struct.
                //
                // We cast the void pointer to a TypeId as it's the minimum
                // amount of memory we need in order to perform the test.
                let maybe_type_id = maybe_cell as *const TypeId;
                *maybe_type_id == TypeId::of::<T>()
            }
        }
    }
}

impl<T> fmt::Debug for WrenCell<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WrenCell")
            .field("type_id", &self.type_id)
            .field("cell", &self.cell)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_is_type() {
        let mut a = WrenCell::new(7.0f64);
        let void_ptr = &mut a as *mut _ as *mut c_void;
        assert!(WrenCell::<f64>::is_type(void_ptr));

        #[allow(dead_code)]
        struct Test {
            s: String,
            a: [f64; 4],
        }

        let mut b = WrenCell::new(Test {
            s: "test string b".to_owned(),
            a: [1.3, 2.7, 3.11, 4.21],
        });
        let void_ptr = &mut b as *mut _ as *mut c_void;
        assert!(WrenCell::<Test>::is_type(void_ptr));
        assert!(!WrenCell::<Option<Test>>::is_type(void_ptr));
        assert!(!WrenCell::<u64>::is_type(void_ptr));
        assert!(!WrenCell::<TypeId>::is_type(void_ptr));
        assert!(!WrenCell::<()>::is_type(void_ptr));
    }

    #[test]
    fn test_from_ptr() {
        struct Test {
            s: String,
            a: [i32; 4],
        }

        let mut a = WrenCell::new(Test {
            s: "test string a".to_owned(),
            a: [1, 2, 3, 4],
        });
        let void_ptr = &mut a as *mut _ as *mut c_void;

        assert!(unsafe { WrenCell::<u64>::from_ptr(void_ptr).is_err() });
        assert!(unsafe { WrenCell::<u64>::from_ptr_mut(void_ptr).is_err() });
        assert!(unsafe { WrenCell::<TypeId>::from_ptr(void_ptr).is_err() });
        assert!(unsafe { WrenCell::<TypeId>::from_ptr_mut(void_ptr).is_err() });

        {
            let retrieved = unsafe { WrenCell::<Test>::from_ptr_mut(void_ptr) };
            assert!(retrieved.is_ok());
            let mut inner = retrieved.unwrap().borrow_mut();
            assert_eq!(inner.s.as_str(), "test string a");
            assert_eq!(inner.a, [1, 2, 3, 4]);
            inner.s = "mutated".into();
            inner.a = [2, 3, 4, 5];
        }

        {
            // Note: cell has inner mutability.
            let retrieved = unsafe { WrenCell::<Test>::from_ptr(void_ptr) };
            assert!(retrieved.is_ok());
            let inner = retrieved.unwrap().borrow_mut();
            // Mutated in previous step
            assert_eq!(inner.s.as_str(), "mutated");
            assert_eq!(inner.a, [2, 3, 4, 5]);
        }
    }
}
