use crate::{
    bindings,
    class::WrenCell,
    errors::{WrenError, WrenResult},
    types::WrenType,
    WrenContext,
};
use std::{
    ffi::{CStr, CString},
    os::raw::c_void,
};

/// Helper macro for common verifications.
macro_rules! verify_slot {
    ($ctx:ident, $n:ident, $t:path) => {
        if $n < 0 {
            return Err($crate::errors::WrenError::SlotOutOfBounds($n as i32));
        }
        if $n >= $ctx.slot_count() as i32 {
            return Err($crate::errors::WrenError::SlotOutOfBounds($n as i32));
        }
        {
            let slot_type = $ctx.slot_type($n as usize);
            if slot_type != Some($t) {
                return Err($crate::errors::WrenError::SlotType {
                    actual: slot_type.unwrap(),
                    expected: $t,
                });
            }
        }
    };
}

pub trait FromWren<'wren> {
    type Output: Sized;

    fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output>;
}

impl<'wren> FromWren<'wren> for bool {
    type Output = Self;

    fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output> {
        verify_slot!(ctx, slot_num, WrenType::Bool);
        Ok(unsafe { bindings::wrenGetSlotBool(ctx.vm_ptr(), slot_num) })
    }
}

macro_rules! impl_from_wren_num {
    ($t:ty) => {
        impl<'wren> FromWren<'wren> for $t {
            type Output = Self;

            #[inline]
            fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output> {
                verify_slot!(ctx, slot_num, WrenType::Number);
                Ok(unsafe { bindings::wrenGetSlotDouble(ctx.vm_ptr(), slot_num) } as Self)
            }
        }
    };
}

impl_from_wren_num!(i8);
impl_from_wren_num!(i16);
impl_from_wren_num!(i32);
impl_from_wren_num!(i64);
impl_from_wren_num!(u8);
impl_from_wren_num!(u16);
impl_from_wren_num!(u32);
impl_from_wren_num!(u64);
impl_from_wren_num!(f32);
impl_from_wren_num!(f64);

impl<'wren> FromWren<'wren> for String {
    type Output = Self;

    fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output> {
        <&str as FromWren>::get_slot(ctx, slot_num).map(|s| s.to_owned())
    }
}

/// Pretty risky. If we borrow a Wren string that gets garbage collected...
impl<'wren> FromWren<'wren> for &'wren str {
    type Output = Self;

    fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output> {
        verify_slot!(ctx, slot_num, WrenType::String);

        unsafe {
            let char_ptr = bindings::wrenGetSlotString(ctx.vm_ptr(), slot_num);
            if char_ptr.is_null() {
                Err(WrenError::NullPtr)
            } else {
                let c_str = CStr::from_ptr(char_ptr);
                c_str.to_str().map_err(WrenError::Utf8)
            }
        }
    }
}

/// Does nothing.
impl<'wren> FromWren<'wren> for () {
    type Output = Self;

    #[inline]
    fn get_slot(_ctx: &WrenContext, _slot_num: i32) -> WrenResult<Self::Output> {
        Ok(())
    }
}

/// Wrapped in two `Option`s. The first will be unwrapped before calling
/// the foreign method, and must be replaced with a WrenResult in the
/// future. The second is the actual value passed to the foreign method,
/// because it literally takes `Option<T>`.
impl<'wren, T> FromWren<'wren> for Option<T>
where
    T: FromWren<'wren>,
{
    type Output = Option<T::Output>;

    #[inline]
    fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output> {
        if slot_num < 0 {
            return Err(WrenError::SlotOutOfBounds(slot_num));
        }
        if slot_num >= ctx.slot_count() as i32 {
            return Err(WrenError::SlotOutOfBounds(slot_num));
        }

        match ctx.slot_type(slot_num as usize) {
            Some(WrenType::Null) => Ok(None),
            _ => T::get_slot(ctx, slot_num).map(Some),
        }
    }
}

impl<'wren, T> FromWren<'wren> for WrenCell<T>
where
    T: 'static,
{
    type Output = &'wren WrenCell<T>;

    #[inline]
    fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output> {
        verify_slot!(ctx, slot_num, WrenType::Foreign);
        let void_ptr: *const c_void = unsafe { bindings::wrenGetSlotForeign(ctx.vm_ptr(), slot_num) as _ };
        unsafe { WrenCell::<T>::from_ptr(void_ptr) }
    }
}

/// Needs an explicit implementation, otherwise the type checker
/// picks `WrenForeignClass` for some reason.
impl<'wren, T> FromWren<'wren> for &WrenCell<T>
where
    T: 'static,
{
    type Output = &'wren WrenCell<T>;

    #[inline]
    fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output> {
        WrenCell::<T>::get_slot(ctx, slot_num)
    }
}

// FIXME: Move to class.rs
// FIXME: Check if slot 0 is type UNKNOWN and print helpful error. Can be instance method call on static Rust func, or missing `foreign` on class.
/// Needs an explicit implementation, otherwise the type checker
/// picks `WrenForeignClass` for some reason.
impl<'wren, T> FromWren<'wren> for &mut WrenCell<T>
where
    T: 'static,
{
    type Output = &'wren mut WrenCell<T>;

    #[inline]
    fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output> {
        verify_slot!(ctx, slot_num, WrenType::Foreign);
        let void_ptr: *mut c_void = unsafe { bindings::wrenGetSlotForeign(ctx.vm_ptr(), slot_num) as _ };
        unsafe { WrenCell::<T>::from_ptr_mut(void_ptr) }
    }
}

/// A type that can be passed to a Wren VM via a slot.
pub trait ToWren {
    /// Moves the value into a slot in the VM.
    fn put(self, ctx: &mut WrenContext, slot: i32);

    fn size_hint(&self) -> usize {
        1
    }
}

impl ToWren for bool {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        unsafe { bindings::wrenSetSlotBool(ctx.vm_ptr(), slot, self) }
    }
}

macro_rules! impl_to_wren_num {
    ($t:ty) => {
        impl ToWren for $t {
            #[inline]
            fn put(self, ctx: &mut WrenContext, slot: i32) {
                unsafe { bindings::wrenSetSlotDouble(ctx.vm_ptr(), slot, self as f64) }
            }
        }
    };
}

impl_to_wren_num!(i8);
impl_to_wren_num!(i16);
impl_to_wren_num!(i32);
impl_to_wren_num!(i64);
impl_to_wren_num!(u8);
impl_to_wren_num!(u16);
impl_to_wren_num!(u32);
impl_to_wren_num!(u64);
impl_to_wren_num!(f32);
impl_to_wren_num!(f64);

impl ToWren for String {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        // Wren copies the contents of the given string.
        let c_string = CString::new(self).expect("String contains a null byte");
        unsafe { bindings::wrenSetSlotString(ctx.vm_ptr(), slot, c_string.as_ptr()) }
    }
}

impl ToWren for &str {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        // Wren copies the contents of the given string.
        // We have two copies here, first &str to CString, then Wren allocateString.
        let c_string = CString::new(self).expect("String contains a null byte");
        unsafe { bindings::wrenSetSlotString(ctx.vm_ptr(), slot, c_string.as_ptr()) }
    }
}

impl ToWren for () {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        unsafe { bindings::wrenSetSlotNull(ctx.vm_ptr(), slot) }
    }
}

impl<T> ToWren for Option<T>
where
    T: ToWren,
{
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        match self {
            Some(val) => val.put(ctx, slot),
            None => unsafe { bindings::wrenSetSlotNull(ctx.vm_ptr(), slot) },
        }
    }
}

// Wren maximum function arguments is 16
rust_wren_derive::generate_tuple_to_wren!(A);
rust_wren_derive::generate_tuple_to_wren!(A, B);
rust_wren_derive::generate_tuple_to_wren!(A, B, C);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F, G);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F, G, H);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F, G, H, I);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F, G, H, I, J);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F, G, H, I, J, K);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F, G, H, I, J, K, L);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F, G, H, I, J, K, L, M);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
rust_wren_derive::generate_tuple_to_wren!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
