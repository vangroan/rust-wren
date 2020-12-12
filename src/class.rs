use crate::ModuleBuilder;
pub use std::cell::{BorrowError, BorrowMutError, Ref, RefMut};
use std::{any::TypeId, cell::RefCell, os::raw::c_void};

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
    pub fn new(inner: T) -> Self {
        WrenCell {
            type_id: TypeId::of::<T>(),
            cell: RefCell::new(inner),
        }
    }

    pub fn from_cell(cell: RefCell<T>) -> Self {
        WrenCell {
            type_id: TypeId::of::<T>(),
            cell,
        }
    }

    #[inline]
    pub fn from_ptr<'a>(maybe_cell: *const c_void) -> Option<&'a Self> {
        if maybe_cell.is_null() || !Self::is_type(maybe_cell) {
            None
        } else {
            unsafe { (maybe_cell as *const Self).as_ref() }
        }
    }

    #[inline]
    pub fn from_ptr_mut<'a>(maybe_cell: *mut c_void) -> Option<&'a mut Self> {
        if maybe_cell.is_null() || !Self::is_type(maybe_cell) {
            None
        } else {
            unsafe { (maybe_cell as *mut Self).as_mut() }
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
    pub fn try_borrow(&self) -> Result<Ref<'_, T>, BorrowError> {
        self.cell.try_borrow()
    }

    #[inline]
    pub fn try_borrow_mut(&self) -> Result<RefMut<'_, T>, BorrowMutError> {
        self.cell.try_borrow_mut()
    }

    #[inline]
    pub fn is_type(maybe_cell: *const c_void) -> bool {
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

        assert!(WrenCell::<u64>::from_ptr(void_ptr).is_none());
        assert!(WrenCell::<u64>::from_ptr_mut(void_ptr).is_none());
        assert!(WrenCell::<TypeId>::from_ptr(void_ptr).is_none());
        assert!(WrenCell::<TypeId>::from_ptr_mut(void_ptr).is_none());

        {
            let retrieved = WrenCell::<Test>::from_ptr_mut(void_ptr);
            assert!(retrieved.is_some());
            let mut inner = retrieved.unwrap().borrow_mut();
            assert_eq!(inner.s.as_str(), "test string a");
            assert_eq!(inner.a, [1, 2, 3, 4]);
            inner.s = "mutated".into();
            inner.a = [2, 3, 4, 5];
        }

        {
            // Note: cell has inner mutability.
            let retrieved = WrenCell::<Test>::from_ptr(void_ptr);
            assert!(retrieved.is_some());
            let inner = retrieved.unwrap().borrow_mut();
            // Mutated in previous step
            assert_eq!(inner.s.as_str(), "mutated");
            assert_eq!(inner.a, [2, 3, 4, 5]);
        }
    }
}
