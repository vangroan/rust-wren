use crate::{bindings, class::WrenForeignClass, types::WrenType, WrenContext};
use std::cell::RefCell;

/// Helper macro for common verifications.
macro_rules! verify_slot {
    ($ctx:ident, $n:ident, $t:path) => {
        if $n < 0 {
            return None;
        }
        if $n >= $ctx.slot_count() as i32 {
            return None;
        }
        if $ctx.slot_type($n as usize) != Some($t) {
            return None;
        }
    };
}

pub trait FromWren<'wren> {
    type Output: Sized;

    fn get_slot(ctx: &mut WrenContext, slot_num: i32) -> Option<Self::Output>;
}

impl<'wren> FromWren<'wren> for bool {
    type Output = Self;

    fn get_slot(ctx: &mut WrenContext, slot_num: i32) -> Option<Self::Output> {
        verify_slot!(ctx, slot_num, WrenType::Bool);
        Some(unsafe { bindings::wrenGetSlotBool(ctx.vm, slot_num) })
    }
}

impl<'wren> FromWren<'wren> for f64 {
    type Output = Self;

    fn get_slot(ctx: &mut WrenContext, slot_num: i32) -> Option<Self::Output> {
        verify_slot!(ctx, slot_num, WrenType::Number);
        Some(unsafe { bindings::wrenGetSlotDouble(ctx.vm, slot_num) })
    }
}

// impl<T> FromWren for T
// where
//     T: WrenForeignClass + Copy,
// {
//     type Output = Self;

//     fn get_slot(ctx: &mut WrenContext, slot_num: i32) -> Option<Self::Output> {
//         verify_slot!(ctx, slot_num, WrenType::Foreign);
//         let foreign_ptr: *mut RefCell<Self> = unsafe { bindings::wrenGetSlotForeign(ctx.vm, slot_num) as _ };
//         let foreign_mut: &mut RefCell<Self> = unsafe { foreign_ptr.as_mut().unwrap() };
//         Some(foreign_mut)
//     }
// }

/// Does nothing.
impl<'wren> FromWren<'wren> for () {
    type Output = ();

    #[inline]
    fn get_slot(_ctx: &mut WrenContext, _slot_num: i32) -> Option<Self::Output> {
        Some(())
    }
}

impl<'wren, T> FromWren<'wren> for T
where
    T: 'wren + WrenForeignClass,
{
    type Output = &'wren mut RefCell<Self>;

    fn get_slot(ctx: &mut WrenContext, slot_num: i32) -> Option<Self::Output> {
        verify_slot!(ctx, slot_num, WrenType::Foreign);
        let foreign_ptr: *mut RefCell<Self> =
            unsafe { bindings::wrenGetSlotForeign(ctx.vm, slot_num) as _ };
        let foreign_mut: &mut RefCell<Self> = unsafe { foreign_ptr.as_mut().unwrap() };
        Some(foreign_mut)
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
        unsafe { bindings::wrenSetSlotBool(ctx.vm, slot, self) }
    }
}

impl ToWren for f32 {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        // Wren has no single float type.
        unsafe { bindings::wrenSetSlotDouble(ctx.vm, slot, self as f64) }
    }
}

impl ToWren for f64 {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        // Wren has no single float type.
        unsafe { bindings::wrenSetSlotDouble(ctx.vm, slot, self) }
    }
}

impl ToWren for () {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        unsafe { bindings::wrenSetSlotNull(ctx.vm, slot) }
    }
}

impl<T> ToWren for Option<T>
where
    T: ToWren,
{
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        match self {
            Some(val) => val.put(ctx, slot),
            None => unsafe { bindings::wrenSetSlotNull(ctx.vm, slot) },
        }
    }
}

impl<A, B> ToWren for (A, B)
where
    A: ToWren,
    B: ToWren,
{
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        let (a, b) = self;
        A::put(a, ctx, slot);
        B::put(b, ctx, slot + 1);
    }

    fn size_hint(&self) -> usize {
        2
    }
}

impl<A, B, C> ToWren for (A, B, C)
where
    A: ToWren,
    B: ToWren,
    C: ToWren,
{
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        let (a, b, c) = self;
        A::put(a, ctx, slot);
        B::put(b, ctx, slot + 1);
        C::put(c, ctx, slot + 2);
    }

    fn size_hint(&self) -> usize {
        3
    }
}
