use crate::{
    bindings,
    errors::{WrenError, WrenResult},
    handle::WrenHandle,
    types::WrenType,
    value::{FromWren, ToWren},
    vm::WrenContext,
};
use std::{fmt, os::raw::c_int};

/// Handle to a list in Wren.
///
/// Requires the [`WrenContext`] that owns the list
/// to perform operations on it.
pub struct WrenList(WrenHandle);

impl WrenList {
    /// The type when the value is in a slot.
    pub const WREN_TYPE: bindings::WrenType = bindings::WrenType_WREN_TYPE_LIST;

    /// Create a new, empty list in the given Wren VM.
    pub fn new(ctx: &mut WrenContext) -> Self {
        ctx.ensure_slots(1);
        let destructor_queue = ctx.destructor_sender();

        unsafe {
            bindings::wrenSetSlotNewList(ctx.vm_ptr(), 0);
            let handle_ptr = bindings::wrenGetSlotHandle(ctx.vm_ptr(), 0);
            let handle = WrenHandle::from_raw(handle_ptr, destructor_queue);
            WrenList(handle)
        }
    }

    /// Create a `WrenList` from a given `WrenHandle`.
    ///
    /// # Safety
    ///
    /// This is unsafe because the handle cannot be
    /// checked if its type is indeed list.
    #[doc(hidden)]
    pub unsafe fn from_handle_unchecked(handle: WrenHandle) -> Self {
        WrenList(handle)
    }

    /// Create a new list in Wren, copying the contents of the
    /// given slice into it.
    ///
    /// Returns a handle to the created list.
    pub fn from_slice<T: ToWren + Clone>(ctx: &mut WrenContext, data: &[T]) -> WrenResult<Self> {
        // Slot for list receiver and item
        ctx.ensure_slots(2);
        let destructor_queue = ctx.destructor_sender();

        unsafe {
            bindings::wrenSetSlotNewList(ctx.vm_ptr(), 0);
            let handle_ptr = bindings::wrenGetSlotHandle(ctx.vm_ptr(), 0);
            let handle = WrenHandle::from_raw(handle_ptr, destructor_queue);

            for el in data.iter() {
                <T as ToWren>::put(el.clone(), ctx, 1);
                bindings::wrenInsertInList(ctx.vm_ptr(), 0, -1, 1);
            }

            Ok(WrenList::from_handle_unchecked(handle))
        }
    }

    /// Create a new list in Wren, copying the contents of the
    /// given vector into it.
    ///
    /// Returns a handle to the created list.
    pub fn from_vec<T: ToWren>(ctx: &mut WrenContext, data: Vec<T>) -> WrenResult<Self> {
        // Slot for list receiver and item
        ctx.ensure_slots(2);
        let destructor_queue = ctx.destructor_sender();

        unsafe {
            bindings::wrenSetSlotNewList(ctx.vm_ptr(), 0);
            let handle_ptr = bindings::wrenGetSlotHandle(ctx.vm_ptr(), 0);
            let handle = WrenHandle::from_raw(handle_ptr, destructor_queue);

            for el in data.into_iter() {
                <T as ToWren>::put(el, ctx, 1);
                bindings::wrenInsertInList(ctx.vm_ptr(), 0, -1, 1);
            }

            Ok(WrenList::from_handle_unchecked(handle))
        }
    }

    /// Appends an item to the back of the collection.
    pub fn push<T: ToWren>(&mut self, ctx: &mut WrenContext, item: T) {
        // Slot for list and item
        ctx.ensure_slots(2);
        ToWren::put(item, ctx, 1);

        unsafe {
            bindings::wrenSetSlotHandle(ctx.vm_ptr(), 0, self.0.raw_ptr().as_ptr());

            // According to Wren list interface, inserting to index -1 is appending.
            bindings::wrenInsertInList(ctx.vm_ptr(), 0, -1, 1);
        }
    }

    #[inline(always)]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self, ctx: &mut WrenContext) -> usize {
        ctx.ensure_slots(1);
        unsafe {
            bindings::wrenSetSlotHandle(ctx.vm_ptr(), 0, self.0.raw_ptr().as_ptr());
            bindings::wrenGetListCount(ctx.vm_ptr(), 0) as usize
        }
    }

    /// Get length of list without ensuring the number of slots.
    ///
    /// # Safety
    ///
    /// If there are not enough slots, the value will be writing the length outside
    /// of the slots array and corrupt memory.
    pub unsafe fn len_unchecked(&self, ctx: &mut WrenContext) -> usize {
        bindings::wrenSetSlotHandle(ctx.vm_ptr(), 0, self.0.raw_ptr().as_ptr());
        bindings::wrenGetListCount(ctx.vm_ptr(), 0) as usize
    }

    #[inline(always)]
    pub fn is_empty(&self, ctx: &mut WrenContext) -> bool {
        self.len(ctx) == 0
    }

    pub fn set<T: ToWren>(&mut self, ctx: &mut WrenContext, index: usize, item: T) {
        // Wren does not do bounds check
        if index >= self.len(ctx) {
            panic!("index out of bounds");
        }

        // Slot for list and item
        ctx.ensure_slots(2);
        ToWren::put(item, ctx, 1);

        unsafe {
            bindings::wrenSetSlotHandle(ctx.vm_ptr(), 0, self.0.raw_ptr().as_ptr());
            bindings::wrenSetListElement(ctx.vm_ptr(), 0, index as c_int, 1);
        }
    }

    // TODO: Result<Option<T::Output>> can be flattened by having an out-of-bounds WrenError variant
    pub fn get<'wren, T>(&self, ctx: &'wren mut WrenContext, index: usize) -> Result<Option<T::Output>, WrenError>
    where
        T: FromWren<'wren>,
    {
        // Wren does not do bounds check
        if index >= self.len(ctx) {
            return Ok(None);
        }

        ctx.ensure_slots(2);

        unsafe {
            bindings::wrenSetSlotHandle(ctx.vm_ptr(), 0, self.0.raw_ptr().as_ptr());
            bindings::wrenGetListElement(ctx.vm_ptr(), 0, index as c_int, 1);
        }

        <Option<T> as FromWren>::get_slot(ctx, 1)
    }

    /// Copies the contents of the list into a new `Vec`.
    ///
    /// # Errors
    ///
    /// Returns `WrenError` if an element in the list does not
    /// match the type of `T::Output`.
    pub fn to_vec<'wren, T>(&self, ctx: &mut WrenContext) -> Result<Vec<T::Output>, WrenError>
    where
        T: FromWren<'wren>,
    {
        let mut result = vec![];

        ctx.ensure_slots(2);
        let size = unsafe { self.len_unchecked(ctx) };

        for index in 0..size {
            unsafe {
                bindings::wrenSetSlotHandle(ctx.vm_ptr(), 0, self.0.raw_ptr().as_ptr());
                bindings::wrenGetListElement(ctx.vm_ptr(), 0, index as c_int, 1);
            }

            let element = <T as FromWren>::get_slot(ctx, 1)?;
            result.push(element);
        }

        Ok(result)
    }

    /// Clones the contents of the list to the given buffer.
    ///
    /// Returns the number of elements copied.
    ///
    /// # Errors
    ///
    /// Will abort the clone and return an error if an element in the list
    /// cannot be converted to tyep `T`.
    pub fn clone_to<'wren, T>(&self, ctx: &mut WrenContext, buf: &mut [T::Output]) -> Result<usize, WrenError>
    where
        T: FromWren<'wren>,
    {
        ctx.ensure_slots(2);
        let list_size = unsafe { self.len_unchecked(ctx) };
        let buf_size = buf.len();
        let size = ::std::cmp::min(list_size, buf_size);

        for index in 0..size {
            unsafe {
                bindings::wrenSetSlotHandle(ctx.vm_ptr(), 0, self.0.raw_ptr().as_ptr());
                bindings::wrenGetListElement(ctx.vm_ptr(), 0, index as c_int, 1);
            }

            let element = <T as FromWren>::get_slot(ctx, 1)?;
            buf[index] = element;
        }

        Ok(size)
    }

    // fn clone_from<T>(&self)

    // TODO: There is no remove element in Wren API
}

impl<'wren> FromWren<'wren> for WrenList {
    type Output = WrenList;

    fn get_slot(ctx: &WrenContext, list_slot: i32) -> WrenResult<Self::Output> {
        if ctx.slot_type(list_slot as usize) != Some(WrenType::List) {
            return Err(WrenError::SlotType {
                actual: ctx.slot_type(list_slot as usize).unwrap(),
                expected: WrenType::List,
            });
        }

        unsafe {
            let list_handle = bindings::wrenGetSlotHandle(ctx.vm_ptr(), list_slot);
            if list_handle.is_null() {
                return Err(WrenError::NullPtr);
            }

            let destructors = ctx.destructor_sender();

            Ok(WrenList(WrenHandle::from_raw(list_handle, destructors)))
        }
    }
}

impl fmt::Debug for WrenList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("WrenList").field(unsafe { &self.0.raw_ptr() }).finish()
    }
}

impl ToWren for WrenList {
    fn put(self, ctx: &mut WrenContext, list_slot: i32) {
        ToWren::put(self.0, ctx, list_slot)
    }
}
