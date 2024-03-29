//! Core virtual machine.
use crate::{
    bindings,
    class::{WrenCell, WrenForeignClass},
    errors::{WrenCompileError, WrenError, WrenResult, WrenStackFrame, WrenVmError},
    foreign::{ForeignBindings, ForeignClass, ForeignClassKey, ForeignMethod, ForeignMethodKey},
    handle::{FnSymbolRef, WrenCallRef, WrenHandle, WrenRef},
    list::WrenList,
    module::{ModuleLoader, ModuleResolver},
    runtime, types,
    value::FromWren,
};
use log::trace;
use std::{
    any::TypeId,
    borrow::{Borrow, Cow},
    cell::{Cell, RefCell},
    ffi::CString,
    marker::PhantomData,
    mem,
    os::raw::c_int,
    ptr::{self, NonNull},
    sync::mpsc::{channel, Receiver, Sender},
};

pub struct WrenVm {
    vm: *mut bindings::WrenVM,
    handle_rx: Receiver<*mut bindings::WrenHandle>,
}

impl WrenVm {
    #[must_use = "possible VM errors are contained in the returned result"]
    pub fn interpret(&mut self, module: &str, source: &str) -> WrenResult<()> {
        let result_id: bindings::WrenInterpretResult = {
            let vm = unsafe { self.vm.as_mut().unwrap() };
            let _guard = ContextGuard { vm: self };

            // Wren copies these strings, so they are safe to free.
            let c_module = CString::new(module).expect("Module name contains a null byte");
            let c_source = CString::new(source).expect("Source contains a null byte");
            unsafe { bindings::wrenInterpret(vm, c_module.as_ptr(), c_source.as_ptr()) }
        };

        // self.take_interpret_result(result)
        Self::take_errors(self.vm, result_id)
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

    pub fn context_result<F, R>(&mut self, func: F) -> WrenResult<R>
    where
        F: FnOnce(&mut WrenContext) -> WrenResult<R>,
    {
        let vm = unsafe { self.vm.as_mut().unwrap() };
        let _guard = ContextGuard { vm: self };
        let mut ctx = WrenContext::new(vm);
        func(&mut ctx)
    }

    /// Returns the number of allocated slots.
    #[inline]
    pub fn slot_count(&self) -> i32 {
        unsafe { bindings::wrenGetSlotCount(self.vm) }
    }

    /// Utility function for extracting the concrete [`UserData`] instance from
    /// the given [`WrenVM`].
    ///
    /// Returns `None` if the user data within the VM is null.
    ///
    /// # Safety
    ///
    /// The caller must ensure the given VM pointer is valid and not null.
    pub unsafe fn get_user_data<'a>(vm: *mut bindings::WrenVM) -> Option<&'a mut UserData> {
        if vm.is_null() {
            None
        } else {
            (bindings::wrenGetUserData(vm) as *mut UserData).as_mut()
        }
    }

    /// Given the Wren result enum, build a result or error based
    /// on the VM's state.
    ///
    /// This call is not idempotent. It drains the internal error
    /// queue when the given enum is either compile error or runtime
    /// error.
    #[doc(hidden)]
    pub(crate) fn take_errors(vm: *mut bindings::WrenVM, result_id: bindings::WrenInterpretResult) -> WrenResult<()> {
        let userdata = unsafe { WrenVm::get_user_data(vm).ok_or(WrenError::UserDataNull)? };
        let mut errors = userdata.errors.borrow_mut();

        match result_id {
            bindings::WrenInterpretResult_WREN_RESULT_SUCCESS => Ok(()),
            bindings::WrenInterpretResult_WREN_RESULT_COMPILE_ERROR => {
                if errors.is_empty() {
                    return Err(WrenError::ErrorAbsent(result_id));
                }

                let compile_errors = errors
                    .drain(..)
                    .map(|err| match err {
                        WrenVmError::Compile { module, message, line } => WrenCompileError { module, message, line },
                        err => unreachable!("Unexpected VM error {:?}", err),
                    })
                    .collect::<Vec<_>>();

                Err(WrenError::CompileError(compile_errors))
            }
            bindings::WrenInterpretResult_WREN_RESULT_RUNTIME_ERROR => {
                if errors.is_empty() {
                    return Err(WrenError::ErrorAbsent(result_id));
                }

                let mut message = String::new();
                let mut foreign: Option<Box<dyn ::std::error::Error>> = None;
                let mut stack: Vec<WrenStackFrame> = vec![];

                for err in errors.drain(..) {
                    match err {
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
                        WrenVmError::Foreign(err) => {
                            if foreign.is_some() {
                                panic!("Second foreign error encountered in error queue: {:?}", err);
                            }
                            foreign = Some(err.take_inner())
                        }
                        err => unreachable!("Unexpected VM error {:?}", err),
                    }
                }

                Err(WrenError::RuntimeError {
                    message,
                    foreign,
                    stack,
                })
            }
            _ => unreachable!("Unknown Wren result type: {}", result_id),
        }
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
            log::debug!("Dropping Wren VM: {:?}", self.vm);

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
#[allow(clippy::type_complexity)]
pub struct WrenBuilder {
    foreign: ForeignBindings,
    write_fn: Option<Box<dyn Fn(&str)>>,
    resolver: Option<Box<dyn ModuleResolver>>,
    loader: Option<Box<dyn ModuleLoader>>,
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

    pub fn with_write_fn<F>(mut self, write_fn: F) -> Self
    where
        F: Fn(&str) + 'static,
    {
        self.write_fn = Some(Box::new(write_fn));
        self
    }

    pub fn with_module_resolver<T>(mut self, resolver: T) -> Self
    where
        T: 'static + ModuleResolver,
    {
        self.resolver = Some(Box::new(resolver));
        self
    }

    pub fn with_module_loader<T>(mut self, loader: T) -> Self
    where
        T: 'static + ModuleLoader,
    {
        self.loader = Some(Box::new(loader));
        self
    }

    /// By default print to stdout.
    fn default_write_fn() -> Box<dyn Fn(&str) + 'static> {
        Box::new(|s| print!("{}", s))
    }

    pub fn build(self) -> WrenVm {
        // Wren handle pointers that need to be released.
        let (handle_tx, handle_rx) = channel();

        let mut config = unsafe {
            let mut uninit_config = mem::MaybeUninit::<bindings::WrenConfiguration>::zeroed();
            bindings::wrenInitConfiguration(uninit_config.as_mut_ptr());
            uninit_config.assume_init()
        };

        let WrenBuilder {
            foreign,
            write_fn,
            resolver,
            loader,
        } = self;

        config.resolveModuleFn = if resolver.is_some() {
            Some(runtime::resolve_module)
        } else {
            None
        };
        config.loadModuleFn = if loader.is_some() {
            Some(runtime::load_module)
        } else {
            None
        };
        config.reallocateFn = Some(runtime::wren_reallocate);
        config.writeFn = Some(runtime::write_function);
        config.errorFn = Some(runtime::error_function);

        let user_data = UserData {
            foreign,
            handle_tx,
            resolver,
            loader,
            errors: RefCell::new(Vec::new()),
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

        log::debug!("Created Wren VM: {:?}", vm);
        WrenVm { vm, handle_rx }
    }
}

pub struct WrenContext<'wren> {
    pub(crate) vm: Cell<NonNull<bindings::WrenVM>>,
    /// Channel of Wren handles that need to be released in the VM.
    handle_tx: Sender<*mut bindings::WrenHandle>,
    _marker: PhantomData<&'wren bindings::WrenVM>,
}

impl<'wren> WrenContext<'wren> {
    pub fn new(vm: &'wren mut bindings::WrenVM) -> Self {
        let userdata = unsafe { WrenVm::get_user_data(vm).unwrap() };
        let handle_tx = userdata.handle_tx.clone();

        WrenContext {
            vm: unsafe { Cell::new(NonNull::new_unchecked(vm)) },
            handle_tx,
            _marker: PhantomData,
        }
    }

    /// Retrieve a raw pointer to the inner VM.
    ///
    /// Intended to be used by generated code.
    #[doc(hidden)]
    #[inline(always)]
    pub fn vm_ptr(&self) -> *mut bindings::WrenVM {
        self.vm.get().as_ptr()
    }

    #[inline]
    pub fn get_slot<T>(&self, index: i32) -> WrenResult<T::Output>
    where
        T: FromWren<'wren>,
    {
        T::get_slot(self, index)
    }

    #[inline]
    pub fn get_foreign_cell<T>(&self, index: i32) -> Option<&'wren WrenCell<T>>
    where
        T: 'static + WrenForeignClass,
    {
        let foreign_ptr: *mut WrenCell<T> = unsafe { bindings::wrenGetSlotForeign(self.vm_ptr(), index) as _ };
        let foreign_mut: &mut WrenCell<T> = unsafe { foreign_ptr.as_mut().unwrap() };
        Some(foreign_mut)
    }

    /// Retrieve the current number of slots.
    #[inline]
    pub fn slot_count(&self) -> usize {
        let count: c_int = unsafe { bindings::wrenGetSlotCount(self.vm_ptr()) };
        count as usize
    }

    /// Retrieve the type of the value stored in the given slot.
    ///
    /// Returns `None` if the slot index is out of bounds.
    #[inline]
    pub fn slot_type(&self, slot_num: usize) -> Option<types::WrenType> {
        if slot_num >= self.slot_count() {
            None
        } else {
            let ty = unsafe { bindings::wrenGetSlotType(self.vm_ptr(), slot_num as c_int) };
            Some(ty.into())
        }
    }

    /// Grow the slots array to match the given size.
    #[inline]
    pub fn ensure_slots(&self, slot_size: usize) {
        unsafe {
            bindings::wrenEnsureSlots(self.vm_ptr(), slot_size as c_int);
        }
    }

    /// Retrieves the value of a variable from the top level of module,
    /// and returns it as a untyped handle.
    ///
    /// # Safety
    ///
    /// Currently this is unsafe. If the module or variable do not exist, we get undefined behaviour.
    ///
    /// See:
    /// - [#717 When using wrenGetVariable, it now returns an int to inform you of failure](https://github.com/wren-lang/wren/pull/717)
    /// - [#601 wrenGetVariable does not seem to return a sane value](https://github.com/wren-lang/wren/issues/601)
    pub fn get_var(&self, module: &str, name: &str) -> WrenResult<WrenRef<'wren>> {
        trace!("get_var({}, {})", module, name);
        let c_module = CString::new(module).expect("Module name contains a null byte");
        let c_name = CString::new(name).expect("Name name contains a null byte");

        let module_exists = unsafe { bindings::wrenHasModule(self.vm_ptr(), c_module.as_ptr()) };
        if !module_exists {
            return Err(WrenError::ModuleNotFound(module.to_string()));
        }

        let var_exists = unsafe { bindings::wrenHasVariable(self.vm_ptr(), c_module.as_ptr(), c_name.as_ptr()) };
        if !var_exists {
            return Err(WrenError::VariableNotFound(name.to_string()));
        }
        trace!("Module and variable exist {}.{}", module, name);

        self.ensure_slots(1);

        unsafe {
            bindings::wrenGetVariable(self.vm_ptr(), c_module.as_ptr(), c_name.as_ptr(), 0);
        }
        trace!("Retrieved variable {}.{} of type {:?}", module, name, self.slot_type(0));

        // If the module or variable don't exist, there's junk in the slot.
        self.get_slot::<WrenRef<'wren>>(0)
    }

    /// Retrieve a list from the top level of the given module.
    ///
    /// # Errors
    ///
    /// Returns and error when:
    ///
    /// - Either the module or varable don't exist.
    /// - The variable is not of type list.
    /// - Wren returned a null pointer as the handle.
    pub fn get_list(&self, module: &str, name: &str) -> WrenResult<WrenList> {
        let wren_ref = self.get_var(module, name)?;
        let wren_handle: WrenHandle = wren_ref.leak()?;
        // FIXME: Slot type check
        let list = unsafe { WrenList::from_handle_unchecked(wren_handle) };
        Ok(list)
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
    pub fn has_var(&self, module: &str, name: &str) -> bool {
        trace!("has_var({}, {})", module, name);
        let c_module = CString::new(module).expect("Module name contains a null byte");
        let c_name = CString::new(name).expect("Name name contains a null byte");

        let module_exists = unsafe { bindings::wrenHasModule(self.vm_ptr(), c_module.as_ptr()) };
        if !module_exists {
            false
        } else {
            unsafe { bindings::wrenHasVariable(self.vm_ptr(), c_module.as_ptr(), c_name.as_ptr()) }
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
    pub fn has_module(&self, module: &str) -> bool {
        trace!("has_module({})", module);
        let c_module = CString::new(module).expect("Module name contains a null byte");

        unsafe { bindings::wrenHasModule(self.vm_ptr(), c_module.as_ptr()) }
    }

    /// Looks up a class or object instance method and returns a call handle reference.
    ///
    /// # Errors
    ///
    /// Will return an error if the given variable doesn't exist, or the function signature has
    /// an invalid format.
    pub fn make_call_ref(&self, module: &str, variable: &str, func_sig: &str) -> WrenResult<WrenCallRef<'wren>> {
        let receiver = self.get_var(module, variable)?;
        let func = FnSymbolRef::compile(self, func_sig)?;
        Ok(WrenCallRef::new(receiver, func))
    }

    /// Retrieve the channel sender for Wren handles that need to be released.
    pub fn destructor_sender(&self) -> Sender<*mut bindings::WrenHandle> {
        self.handle_tx.clone()
    }

    /// Trigger the VM garbage collector.
    pub fn collect_garbage(&mut self) {
        unsafe {
            bindings::wrenCollectGarbage(self.vm_ptr());
        }
    }

    pub fn user_data(&self) -> Option<&UserData> {
        unsafe { WrenVm::get_user_data(self.vm_ptr()).map(|u| &*u) }
    }

    /// Drains VM errors from the user data queue and returns them.
    ///
    /// The input is the result enum of ffi calls to either `wrenInterpret` or `wrenCall`.
    ///
    /// If the given result is success, and the error queue is empty,
    /// the return is `Ok`. If the given result is an error type, the error
    /// queue will be drained and a detailed [`WrenError`](../errors/enum.WrenError.html) is returned.
    ///
    /// This call is not idempotent. The error queue will be drained.
    ///
    /// # Errors
    ///
    /// Unusual errors are caused by a mismatch of the input value and
    /// the internal user data state.
    ///
    /// If the input is success, but there are errors on the queue, a [`WrenError::ResultQueueMismatch`](../errors/enum.WrenError.html#ResultQueueMismatch)
    /// is returned. If the input is runtime or compile error, but the
    /// error queue is empty, then [`WrenError::ErrorAbsent`](../errors/enum.WrenError.html#ErrorAbsent)
    /// is returned.
    pub fn take_errors(&self, result_id: bindings::WrenInterpretResult) -> WrenResult<()> {
        WrenVm::take_errors(self.vm_ptr(), result_id)
    }
}

/// Native functionality that needs to cross the boundary into
/// the VM and back out into native foreign methods.
///
/// User data is the primary mechanism for smuggling custom
/// state into foreign functions, which only receive a raw
/// pointer ot the VM.
pub struct UserData {
    /// Registry of foreign class bindings.
    pub foreign: ForeignBindings,
    /// Queue of Wren handles that need to be released in the VM.
    pub handle_tx: Sender<*mut bindings::WrenHandle>,
    /// Resolver for determining a module's canonical name.
    pub resolver: Option<Box<dyn ModuleResolver>>,
    /// Loader for providing module source code on import.
    pub loader: Option<Box<dyn ModuleLoader>>,
    /// Queue of errors recorded from VM execution.
    /// Drained and consolidated to build [`WrenError`](../errors/struct.WrenError.html).
    pub errors: RefCell<Vec<WrenVmError>>,
    /// Callback to function that can handle `System.print()` calls
    /// from Wren.
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
