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
pub struct WrenList(WrenHandle);

impl WrenList {
    /// The type when the value is in a slot.
    pub const WREN_TYPE: bindings::WrenType = bindings::WrenType_WREN_TYPE_LIST;

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

    pub fn push<T: ToWren>(&mut self, ctx: &mut WrenContext, item: T) {
        // Slot for list and item
        ctx.ensure_slots(2);
        item.put(ctx, 1);

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

    /// Get length of list without ensuring the nuber of slots.
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
        item.put(ctx, 1);

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

/// Copy contents of a Wren list out of Wren and into a `Vec`.
impl<'wren, T> FromWren<'wren> for Vec<T>
where
    T: FromWren<'wren>,
{
    type Output = Vec<T::Output>;

    fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output> {
        if ctx.slot_type(slot_num as usize) != Some(WrenType::List) {
            return Err(WrenError::SlotType {
                actual: ctx.slot_type(slot_num as usize).unwrap(),
                expected: WrenType::List,
            });
        }

        // During foreign calls, we are in the middle of extracting arguments
        // from the slots. We can't use the lower slots to extract the list,
        // it could erase arguments that haven't been extracted yet.
        let slot_count = ctx.slot_count();
        ctx.ensure_slots(slot_count + 2);

        let r0 = slot_count as i32;
        let r1 = r0 + 1;

        unsafe {
            let list_handle = bindings::wrenGetSlotHandle(ctx.vm_ptr(), slot_num);
            let count = bindings::wrenGetListCount(ctx.vm_ptr(), slot_num);

            let mut buf = Vec::<T::Output>::with_capacity(count.abs() as usize);

            for i in 0..count {
                bindings::wrenSetSlotHandle(ctx.vm_ptr(), r0, list_handle);
                bindings::wrenGetListElement(ctx.vm_ptr(), r0, i, r1);
                let element = T::get_slot(ctx, r1)?;
                buf.push(element);
            }

            Ok(buf)
        }
    }
}

impl fmt::Debug for WrenList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("WrenList").field(unsafe { &self.0.raw_ptr() }).finish()
    }
}

/// Put the given `Vec<T>` in Wren.
impl<T> ToWren for Vec<T>
where
    T: ToWren,
{
    fn put(self, ctx: &mut WrenContext, list_slot: i32) {
        // Put can be called while preparing arguments
        // for a Rust call to a Wren function via a
        // function handle.
        //
        // Thus we can't use the lower slots to build this
        // new list because they are being used to setup
        // the call.
        //
        // We assume `wrenEnsureSlots` has been called
        // before with enough slots for the upcoming
        // call, so we extend it for some scratch space
        // to build our new list.
        let slot_count = ctx.slot_count();
        ctx.ensure_slots(slot_count + 1);

        let elem_slot = slot_count as i32;

        unsafe {
            bindings::wrenSetSlotNewList(ctx.vm_ptr(), list_slot);

            for element in self.into_iter() {
                <T as ToWren>::put(element, ctx, elem_slot);

                // Inserting into index -1 means appending to a list,
                // according to the Wren list interface.
                bindings::wrenInsertInList(ctx.vm_ptr(), list_slot, -1, elem_slot);
            }
        }
    }
}
