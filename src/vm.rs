//! Core virtual machine.
use crate::{
    bindings,
    class::{WrenCell, WrenForeignClass},
    errors::{WrenCompileError, WrenError, WrenResult, WrenStackFrame, WrenVmError},
    foreign::{ForeignBindings, ForeignClass, ForeignClassKey, ForeignMethod, ForeignMethodKey},
    handle::{FnSymbolRef, WrenCallRef, WrenRef},
    runtime, types,
    value::FromWren,
};
use log::trace;
use std::{
    any::TypeId,
    borrow::{Borrow, Cow},
    ffi::CString,
    mem,
    os::raw::c_int,
    ptr,
    sync::mpsc::{channel, Receiver, Sender},
};

pub struct WrenVm {
    vm: *mut bindings::WrenVM,
    handle_rx: Receiver<*mut bindings::WrenHandle>,
    error_rx: Receiver<WrenVmError>,
}

impl WrenVm {
    #[must_use = "possible VM errors are contained in the returned result"]
    pub fn interpret(&mut self, module: &str, source: &str) -> WrenResult<()> {
        let result = {
            let vm = unsafe { self.vm.as_mut().unwrap() };
            let _guard = ContextGuard { vm: self };

            // Wren copies these strings, so they are safe to free.
            let c_module = CString::new(module).expect("Module name contains a null byte");
            let c_source = CString::new(source).expect("Source contains a null byte");
            unsafe { bindings::wrenInterpret(vm, c_module.as_ptr(), c_source.as_ptr()) }
        };

        match result {
            bindings::WrenInterpretResult_WREN_RESULT_SUCCESS => Ok(()),
            bindings::WrenInterpretResult_WREN_RESULT_COMPILE_ERROR => {
                let mut errors: Vec<WrenCompileError> = vec![];

                while let Ok(error) = self.error_rx.try_recv() {
                    match error {
                        WrenVmError::Compile { module, message, line } => {
                            errors.push(WrenCompileError { module, message, line })
                        }
                        err @ _ => unreachable!("Unexpected {:?}", err),
                    }
                }

                Err(WrenError::CompileError(errors))
            }
            bindings::WrenInterpretResult_WREN_RESULT_RUNTIME_ERROR => {
                let mut message = String::new();
                let mut foreign: Option<Box<dyn ::std::error::Error>> = None;
                let mut stack: Vec<WrenStackFrame> = vec![];

                while let Ok(error) = self.error_rx.try_recv() {
                    match error {
                        WrenVmError::Runtime { msg } => message.push_str(msg.as_str()),
                        WrenVmError::StackTrace {
                            module,
                            function,
                            line,
                            is_foreign,
                        } => {
                            stack.push(WrenStackFrame {
                                module,
                                function,
                                line,
                                is_foreign,
                            });
                        }
                        WrenVmError::Foreign(err) => foreign = Some(err.take_inner()),
                        err @ _ => unreachable!("Unexpected {:?}", err),
                    }
                }
                Err(WrenError::RuntimeError {
                    message,
                    foreign,
                    stack,
                })
            }
            _ => unreachable!("Unknown Wren result type: {}", result),
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
    pub unsafe fn get_user_data<'a>(vm: *mut bindings::WrenVM) -> Option<&'a mut UserData> {
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
            self.maintain();

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
#[must_use = "Wren VM was not build. Call build() on the builder instance."]
pub struct WrenBuilder {
    foreign: ForeignBindings,
    write_fn: Option<Box<dyn Fn(&str)>>,
}

impl WrenBuilder {
    #[must_use = "must call build on builder to create vm"]
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

    pub fn with_write_fn<F>(mut self, write_fn: F) -> Self
    where
        F: Fn(&str) + 'static,
    {
        self.write_fn = Some(Box::new(write_fn));
        self
    }

    /// By default print to stdout.
    fn default_write_fn() -> Box<dyn Fn(&str) + 'static> {
        Box::new(|s| print!("{}", s))
    }

    pub fn build(self) -> WrenVm {
        // Wren handle pointers that need to be released.
        let (handle_tx, handle_rx) = channel();

        // Errors are piped through a channel to cross the boundary between an extern C callback and outer Rust code.
        let (error_tx, error_rx) = channel();

        let mut config = unsafe {
            let mut uninit_config = mem::MaybeUninit::<bindings::WrenConfiguration>::zeroed();
            bindings::wrenInitConfiguration(uninit_config.as_mut_ptr());
            uninit_config.assume_init()
        };

        config.reallocateFn = Some(runtime::wren_reallocate);
        config.writeFn = Some(runtime::write_function);
        config.errorFn = Some(runtime::error_function);

        let WrenBuilder { foreign, write_fn } = self;
        let user_data = UserData {
            foreign,
            handle_tx,
            error_tx,
            write_fn: write_fn.unwrap_or_else(WrenBuilder::default_write_fn),
        };
        config.userData = Box::into_raw(Box::new(user_data)) as _;
        config.bindForeignMethodFn = Some(ForeignBindings::bind_foreign_method);
        config.bindForeignClassFn = Some(ForeignBindings::bind_foreign_class);

        // WrenVM makes a copy of the configuration. We can
        // discard our copy after creation.
        let vm = unsafe { bindings::wrenNewVM(&mut config) };
        if vm.is_null() {
            panic!("Unexpected null result when creating WrenVM via C");
        }

        WrenVm {
            vm,
            handle_rx,
            error_rx,
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

    /// Retrieve a raw pointer to the inner VM.
    ///
    /// Intended to be used by generated code.
    #[doc(hidden)]
    pub fn vm_ptr(&mut self) -> *mut bindings::WrenVM {
        self.vm as *mut _
    }

    #[inline]
    pub fn get_slot<T>(&mut self, index: i32) -> Option<T::Output>
    where
        T: FromWren<'wren>,
    {
        T::get_slot(self, index)
    }

    #[inline]
    pub fn get_foreign_cell<T>(&mut self, index: i32) -> Option<&'wren WrenCell<T>>
    where
        T: 'static + WrenForeignClass,
    {
        let foreign_ptr: *mut WrenCell<T> = unsafe { bindings::wrenGetSlotForeign(self.vm, index) as _ };
        let foreign_mut: &mut WrenCell<T> = unsafe { foreign_ptr.as_mut().unwrap() };
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
    pub fn get_var(&mut self, module: &str, name: &str) -> Option<WrenRef<'wren>> {
        trace!("get_var({}, {})", module, name);
        let c_module = CString::new(module).expect("Module name contains a null byte");
        let c_name = CString::new(name).expect("Name name contains a null byte");

        let module_exists = unsafe { bindings::wrenHasModule(self.vm, c_module.as_ptr()) };
        if !module_exists {
            return None;
        }

        let var_exists = unsafe { bindings::wrenHasVariable(self.vm, c_module.as_ptr(), c_name.as_ptr()) };
        if !var_exists {
            return None;
        }
        trace!("Module and variable exists {}.{}", module, name);

        self.ensure_slots(1);

        unsafe {
            bindings::wrenGetVariable(self.vm, c_module.as_ptr(), c_name.as_ptr(), 0);
        }
        trace!("Retrieved variable {}.{} of type {:?}", module, name, self.slot_type(0));

        // If the module or variable don't exist, there's junk in the slot.
        self.get_slot::<WrenRef<'wren>>(0)
    }

    /// Checks whether a variable exists.
    ///
    /// # Performance
    ///
    /// Module and variable name strings are copied to Wren.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_wren::prelude::*;
    /// # let mut vm = WrenBuilder::new().build();
    /// # vm.interpret("example", r#"var variableName = 0.0"#).expect("Interpret failed");
    /// vm.context(|ctx| {
    ///     assert!(ctx.has_var("example", "variableName"));
    ///     assert!(!ctx.has_var("example", "doesNotExist"));
    /// });
    /// ```
    pub fn has_var(&mut self, module: &str, name: &str) -> bool {
        trace!("has_var({}, {})", module, name);
        let c_module = CString::new(module).expect("Module name contains a null byte");
        let c_name = CString::new(name).expect("Name name contains a null byte");

        let module_exists = unsafe { bindings::wrenHasModule(self.vm, c_module.as_ptr()) };
        if !module_exists {
            false
        } else {
            unsafe { bindings::wrenHasVariable(self.vm, c_module.as_ptr(), c_name.as_ptr()) }
        }
    }

    /// Checks whether a module exists.
    ///
    /// # Performance
    ///
    /// Copies module and variable name strings to Wren.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_wren::prelude::*;
    /// # let mut vm = WrenBuilder::new().build();
    /// # vm.interpret("example", "").expect("Interpret failed");
    /// vm.context(|ctx| {
    ///     assert!(ctx.has_module("example"));
    ///     assert!(!ctx.has_module("does_not_exist"));
    /// });
    /// ```
    pub fn has_module(&mut self, module: &str) -> bool {
        trace!("has_module({})", module);
        let c_module = CString::new(module).expect("Module name contains a null byte");

        unsafe { bindings::wrenHasModule(self.vm, c_module.as_ptr()) }
    }

    /// Looks up a class or object instance method and returns a call handle reference.
    ///
    /// # Errors
    ///
    /// Will return an error if the given variable doesn't exist, or the function signature has
    /// an invalid format.
    pub fn make_call_ref(&mut self, module: &str, variable: &str, func_sig: &str) -> Option<WrenCallRef<'wren>> {
        let receiver = self.get_var(module, variable)?;
        let func = FnSymbolRef::compile(self, func_sig)?;
        Some(WrenCallRef::new(receiver, func))
    }

    /// Retrieve the channel sender for Wren handles that need to be released.
    pub fn destructor_sender(&self) -> Sender<*mut bindings::WrenHandle> {
        self.handle_tx.clone()
    }

    /// Trigger the VM garbage collector.
    pub fn collect_garbage(&mut self) {
        unsafe {
            bindings::wrenCollectGarbage(self.vm);
        }
    }

    pub fn user_data(&mut self) -> Option<&UserData> {
        unsafe { WrenVm::get_user_data(self.vm).map(|u| &*u) }
    }
}

pub struct UserData {
    pub foreign: ForeignBindings,
    pub handle_tx: Sender<*mut bindings::WrenHandle>,
    pub error_tx: Sender<WrenVmError>,
    pub write_fn: Box<dyn Fn(&str)>,
}

pub struct ModuleBuilder<'a> {
    module: &'a str,
    foreign: &'a mut ForeignBindings,
}

impl<'a> ModuleBuilder<'a> {
    pub fn register<T>(&mut self)
    where
        T: WrenForeignClass,
    {
        T::register(self);
    }

    /// Intended to be used by generated code.
    #[doc(hidden)]
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

    /// Intended to be used by generated code.
    #[doc(hidden)]
    pub fn add_reverse_class_lookup<T>(&mut self)
    where
        T: 'static + WrenForeignClass,
    {
        let key = ForeignClassKey {
            module: self.module.to_owned(),
            class: T::NAME.to_owned(),
        };
        self.foreign.reverse.insert(TypeId::of::<T>(), key);
    }

    /// Intended to be used by generated code.
    #[doc(hidden)]
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
