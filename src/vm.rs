use crate::{
    bindings, class,
    foreign::{ForeignBindings, ForeignClass, ForeignClassKey, ForeignMethod, ForeignMethodKey},
    handle::WrenRef,
    runtime, types,
    value::{FromWren, ToWren},
};
use log::trace;
use std::{
    any::TypeId,
    borrow::{Borrow, Cow},
    cell::RefCell,
    ffi::CString,
    mem,
    os::raw::c_int,
    sync::mpsc::{channel, Receiver, Sender},
    {fmt, ptr},
};

pub struct WrenVm {
    vm: *mut bindings::WrenVM,
    handle_rx: Receiver<*mut bindings::WrenHandle>,
}

impl WrenVm {
    pub fn interpret(&self, module: &str, source: &str) -> WrenResult<()> {
        // Wren copies these strings, so they are safe to free.
        let c_module = CString::new(module).expect("Module name contains a null byte");
        let c_source = CString::new(source).expect("Source contains a null byte");
        let result =
            unsafe { bindings::wrenInterpret(self.vm, c_module.as_ptr(), c_source.as_ptr()) };
        match result {
            bindings::WrenInterpretResult_WREN_RESULT_SUCCESS => Ok(()),
            bindings::WrenInterpretResult_WREN_RESULT_COMPILE_ERROR => Err(WrenError::CompileError),
            bindings::WrenInterpretResult_WREN_RESULT_RUNTIME_ERROR => Err(WrenError::RuntimeError),
            _ => panic!("Unknown Wren result type: {}", result),
        }
    }

    pub fn context<F>(&mut self, func: F)
    where
        F: FnOnce(&mut WrenContext),
    {
        let vm = unsafe { self.vm.as_mut().unwrap() };
        let _guard = ContextGuard { vm: self };
        let mut ctx = WrenContext::new(vm);
        func(&mut ctx);
    }

    /// Returns the number of allocated slots.
    #[inline]
    pub fn slot_count(&self) -> i32 {
        unsafe { bindings::wrenGetSlotCount(self.vm) }
    }

    /// Utility function for extracting the concrete [`UserData`] instance from
    /// the given [`WrenVM`].
    pub(crate) unsafe fn get_user_data<'a>(vm: *mut bindings::WrenVM) -> Option<&'a mut UserData> {
        (bindings::wrenGetUserData(vm) as *mut UserData).as_mut()
    }

    fn maintain(&mut self) {
        trace!("Maintaining WrenVm");
        while let Ok(handle) = self.handle_rx.try_recv() {
            trace!("Release handle {:?}", handle);
            unsafe { bindings::wrenReleaseHandle(self.vm, handle) };
        }
    }
}

impl Drop for WrenVm {
    fn drop(&mut self) {
        if !self.vm.is_null() {
            // Drop boxed user data
            unsafe {
                let c_user_data = bindings::wrenGetUserData(self.vm);
                if !c_user_data.is_null() {
                    let user_data = Box::from_raw(c_user_data as *mut UserData);
                    drop(user_data);
                }
            };

            unsafe { bindings::wrenFreeVM(self.vm) };
            // VM is now deallocated.
            self.vm = ptr::null_mut();
        }
    }
}

/// Scope guard that ensures a [`WrenVm`](struct.WrenVm.html) is maintained
/// when a context ends.
struct ContextGuard<'wren> {
    vm: &'wren mut WrenVm,
}

impl<'wren> Drop for ContextGuard<'wren> {
    fn drop(&mut self) {
        self.vm.maintain();
    }
}

#[derive(Default)]
pub struct WrenBuilder {
    foreign: ForeignBindings,
}

impl WrenBuilder {
    pub fn new() -> WrenBuilder {
        Default::default()
    }

    /// Replaces the foreign bindings with the provided registry.
    pub fn with_foreign(mut self, foreign_bindings: ForeignBindings) -> Self {
        self.foreign = foreign_bindings;
        self
    }

    pub fn with_module<'a, S, F>(mut self, module: S, func: F) -> Self
    where
        S: Into<Cow<'a, str>>,
        F: FnOnce(&mut ModuleBuilder),
    {
        let module_cow = module.into();
        let module_name = module_cow.borrow();
        let mut module_builder = ModuleBuilder {
            module: module_name,
            foreign: &mut self.foreign,
        };
        func(&mut module_builder);
        self
    }

    pub fn build(self) -> WrenVm {
        // Wren handle pointers that need to be released.
        let (handle_tx, handle_rx) = channel();

        let mut config = unsafe {
            let mut uninit_config = mem::MaybeUninit::<bindings::WrenConfiguration>::zeroed();
            bindings::wrenInitConfiguration(uninit_config.as_mut_ptr());
            uninit_config.assume_init()
        };

        config.reallocateFn = Some(runtime::wren_reallocate);
        config.writeFn = Some(runtime::write_function);
        config.errorFn = Some(runtime::error_function);

        let WrenBuilder { foreign } = self;
        let user_data = UserData { foreign, handle_tx };
        config.userData = Box::into_raw(Box::new(user_data)) as _;
        config.bindForeignMethodFn = Some(ForeignBindings::bind_foreign_method);
        config.bindForeignClassFn = Some(ForeignBindings::bind_foreign_class);

        // WrenVM makes a copy of the configuration. We can
        // discard our copy after creation.
        let vm = unsafe { bindings::wrenNewVM(&mut config) };
        if vm.is_null() {
            panic!("Unexpected null result when creating WrenVM via C");
        }

        WrenVm { vm, handle_rx }
    }
}

pub type WrenResult<T> = Result<T, WrenError>;

#[derive(Debug)]
pub enum WrenError {
    CompileError,
    RuntimeError,
}

impl ::std::error::Error for WrenError {}

impl ::std::fmt::Display for WrenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use WrenError::*;
        // Useful error information is output by Wren using the `errorFn`.
        match self {
            CompileError => write!(f, "compile error"),
            RuntimeError => write!(f, "runtime error"),
        }
    }
}

pub struct WrenContext<'wren> {
    pub(crate) vm: &'wren mut bindings::WrenVM,
    /// Channel of Wren handles that need to be released in the VM.
    handle_tx: Sender<*mut bindings::WrenHandle>,
}

impl<'wren> WrenContext<'wren> {
    pub fn new(vm: &'wren mut bindings::WrenVM) -> Self {
        let userdata = unsafe { WrenVm::get_user_data(vm).unwrap() };
        let handle_tx = userdata.handle_tx.clone();

        WrenContext { vm, handle_tx }
    }

    #[inline]
    pub fn get_slot<T>(&mut self, index: i32) -> Option<T::Output>
    where
        T: FromWren<'wren>,
    {
        T::get_slot(self, index)
    }

    #[inline]
    pub fn get_foreign_cell<T>(&mut self, index: i32) -> Option<&'wren mut ::std::cell::RefCell<T>>
    where
        T: class::WrenForeignClass,
    {
        let foreign_ptr: *mut ::std::cell::RefCell<T> =
            unsafe { bindings::wrenGetSlotForeign(self.vm, index) as _ };
        let foreign_mut: &mut ::std::cell::RefCell<T> = unsafe { foreign_ptr.as_mut().unwrap() };
        Some(foreign_mut)
    }

    /// TODO: Change &mut self to &self
    #[inline]
    pub fn slot_count(&mut self) -> usize {
        let count: c_int = unsafe { bindings::wrenGetSlotCount(self.vm) };
        count as usize
    }

    /// TODO: Change &mut self to &self
    #[inline]
    pub fn slot_type(&mut self, slot_num: usize) -> Option<types::WrenType> {
        if slot_num >= self.slot_count() {
            None
        } else {
            let ty = unsafe { bindings::wrenGetSlotType(self.vm, slot_num as c_int) };
            Some(ty.into())
        }
    }

    #[inline]
    pub fn ensure_slots(&mut self, slot_size: usize) {
        unsafe {
            bindings::wrenEnsureSlots(self.vm, slot_size as c_int);
        }
    }

    /// # Safety
    ///
    /// Currently this is unsafe. If the module or variable do not exist, we get undefined behaviour.
    ///
    /// See:
    /// - [#717 When using wrenGetVariable, it now returns an int to inform you of failure](https://github.com/wren-lang/wren/pull/717)
    /// - [#601 wrenGetVariable does not seem to return a sane value](https://github.com/wren-lang/wren/issues/601)
    pub fn var_ref(&mut self, module: &str, name: &str) -> Option<WrenRef<'wren>> {
        let c_module = CString::new(module).expect("Module name contains a null byte");
        let c_name = CString::new(name).expect("Name name contains a null byte");
        self.ensure_slots(1);

        unsafe {
            bindings::wrenGetVariable(self.vm, c_module.as_ptr(), c_name.as_ptr(), 0);
        }

        // If the module or variable don't exist, there's junk in the slot.
        // println!("{}.{} is type {:?} with value {:?}", module, name, self.slot_type(0), self.get_slot::<f64>(0));
        self.get_slot::<WrenRef<'wren>>(0)
    }

    pub fn destructor_sender(&self) -> Sender<*mut bindings::WrenHandle> {
        self.handle_tx.clone()
    }

    /// # Unimplemented!
    ///
    /// This is currently not possible, because we can't get a handle to the foreign
    /// value that has been allocated.
    ///
    /// Moves the given [`WrenForeignClass`] instance, with `'static` type, into Wren.
    ///
    /// Once a value is moved into Wren, it cannot be moved out. Only a
    /// mutable reference can be retrieved as a receiver to one of its Rust
    /// methods.
    ///
    /// The foreign class binding must be registered for the type to be
    /// moved into Wren.
    ///
    /// # Errors
    ///
    /// Returns an error if the foreign class binding cannot be found.
    ///
    /// # Implementation
    ///
    /// Allocates space in Wren's heap to contain the value.
    #[doc(hidden)]
    pub fn create_static<T>(&mut self, class: T) -> WrenRef<'wren>
    where
        T: 'static + class::WrenForeignClass,
    {
        // To allocate a new foreign object, we must first lookup its class.
        let userdata = unsafe { WrenVm::get_user_data(self.vm).unwrap() }; // TODO: Return Error
        let module_name = userdata
            .foreign
            .reverse
            .get(&TypeId::of::<T>())
            .unwrap()
            .module
            .as_str();
        let class_name = <T as class::WrenForeignClass>::NAME;

        // Class declarations are simple variables in Wren.
        let class_ref = self.var_ref(module_name, class_name).unwrap();

        // Prepare for foreign value allocation.
        self.ensure_slots(1);
        ToWren::put(class_ref, self, 0);

        // Wren wants to own the memory containing the data backing the foreign function.
        let wren_ptr: *mut RefCell<T> = unsafe {
            bindings::wrenSetSlotNewForeign(self.vm, 0, 0, ::std::mem::size_of::<T>() as usize) as _
        };
        let wren_val: &mut RefCell<T> = unsafe { wren_ptr.as_mut().unwrap() };

        // Retrieve a handle to the allocated object, to be returned to the caller.
        // FIXME: Slot 0 contains FOREIGN and not HANDLE
        // let handle = unsafe { bindings::wrenGetSlotHandle(self.vm, 0).as_mut().unwrap() };

        // All foreign classes are wrapped in RefCell, because it's possible to
        // borrow the value out of Wren multiple times.
        let mut rust_val = RefCell::new(class);

        // Swap the constructed object on the stack with the heap memory
        // owned by Wren.
        ::std::mem::swap(wren_val, &mut rust_val);

        // After the swap, this now contains the value Wren wrote after it's allocation,
        // which is zeroed. However it's safer to treat it as undefined. Dropping a value
        // that may contain resources like boxes or file handles could cause issues if
        // it's zeroed or filled with junk.
        //
        // We're intentionally disabling drop since it wasn't initialised by Rust.
        ::std::mem::forget(rust_val);

        // Return new value as WrenHandle.
        // WrenRef::new(handle, self.destructor_sender())
        unimplemented!()
    }
}

pub struct UserData {
    pub foreign: ForeignBindings,
    pub handle_tx: Sender<*mut bindings::WrenHandle>,
}

pub struct ModuleBuilder<'a> {
    module: &'a str,
    foreign: &'a mut ForeignBindings,
}

impl<'a> ModuleBuilder<'a> {
    pub fn register<T>(&mut self)
    where
        T: class::WrenForeignClass,
    {
        T::register(self);
    }

    pub fn add_class_binding<S>(&mut self, class: S, binding: ForeignClass)
    where
        S: Into<Cow<'a, str>>,
    {
        let key = ForeignClassKey {
            module: self.module.to_owned(),
            class: class.into().into_owned(),
        };
        self.foreign.classes.insert(key, binding);
    }

    pub fn add_reverse_class_lookup<T>(&mut self)
    where
        T: 'static + class::WrenForeignClass,
    {
        let key = ForeignClassKey {
            module: self.module.to_owned(),
            class: T::NAME.to_owned(),
        };
        self.foreign.reverse.insert(TypeId::of::<T>(), key);
    }

    pub fn add_method_binding<S>(&mut self, class: S, binding: ForeignMethod)
    where
        S: Into<Cow<'a, str>>,
    {
        let key = ForeignMethodKey {
            module: self.module.to_owned(),
            class: class.into().into_owned(),
            sig: binding.sig.clone(),
            is_static: binding.is_static,
        };
        self.foreign.methods.insert(key, binding);
    }
}
