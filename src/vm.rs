use crate::{bindings, class, runtime, types, value::FromWren};
use std::{
    borrow::{Borrow, Cow},
    collections::HashMap,
    ffi::{CStr, CString},
    mem,
    os::raw::{c_char, c_int, c_void},
    {fmt, ptr},
};

pub struct WrenVm {
    vm: *mut bindings::WrenVM,
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

    /// Returns the number of allocated slots.
    #[inline]
    pub fn slot_count(&self) -> i32 {
        unsafe { bindings::wrenGetSlotCount(self.vm) }
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

#[derive(Default)]
pub struct WrenBuilder {
    foreign: ForeignBindings,
}

impl WrenBuilder {
    pub fn new() -> WrenBuilder {
        Default::default()
    }

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

    fn foreign_class_bindings() -> bindings::WrenBindForeignClassFn {
        extern "C" fn bind_foreign_class(
            vm: *mut bindings::WrenVM,
            module: *const c_char,
            class_name: *const c_char,
        ) -> bindings::WrenForeignClassMethods {
            let userdata = unsafe { get_user_data(vm).expect("User data is null") };

            let module = unsafe {
                CStr::from_ptr(module)
                    .to_owned()
                    .to_string_lossy()
                    .to_string()
            };
            let class_name = unsafe {
                CStr::from_ptr(class_name)
                    .to_owned()
                    .to_string_lossy()
                    .to_string()
            };

            println!("binding foreign class {}.{}", module, class_name);

            let (allocate, finalize) = userdata
                .foreign
                .classes
                .0
                .get(&(module, class_name))
                .map(|foreign_class| {
                    let &ForeignClass { allocate, finalize } = foreign_class;
                    println!("Foreign class found.");
                    (Some(allocate), Some(finalize))
                })
                .unwrap_or_else(|| {
                    eprintln!("Warning: Foreign class not found");
                    (None, None)
                });

            bindings::WrenForeignClassMethods { allocate, finalize }
        }

        Some(bind_foreign_class)
    }

    fn foreign_method_bindings() -> bindings::WrenBindForeignMethodFn {
        /// Method used when a foreign method binding is not found.
        extern "C" fn bind_foreign_class(
            vm: *mut bindings::WrenVM,
            module: *const ::std::os::raw::c_char,
            class_name: *const ::std::os::raw::c_char,
            _is_static: bool, // TODO
            signature: *const ::std::os::raw::c_char,
        ) -> bindings::WrenForeignMethodFn {
            let userdata = unsafe { get_user_data(vm).expect("User data is null") };

            let module = unsafe {
                CStr::from_ptr(module)
                    .to_owned()
                    .to_string_lossy()
                    .to_string()
            };
            let class_name = unsafe {
                CStr::from_ptr(class_name)
                    .to_owned()
                    .to_string_lossy()
                    .to_string()
            };
            let signature = unsafe {
                CStr::from_ptr(signature)
                    .to_owned()
                    .to_string_lossy()
                    .to_string()
            };

            let method = userdata
                .foreign
                .methods
                .0
                .get(&(module, class_name, signature))
                .map(|m| m.func);

            if method.is_none() {
                eprintln!("Warning: Foreign method not found");
            }

            method
        }

        Some(bind_foreign_class)
    }

    pub fn build(self) -> WrenVm {
        let mut config = unsafe {
            let mut uninit_config = mem::MaybeUninit::<bindings::WrenConfiguration>::zeroed();
            bindings::wrenInitConfiguration(uninit_config.as_mut_ptr());
            uninit_config.assume_init()
        };

        config.reallocateFn = Some(runtime::wren_reallocate);
        config.writeFn = Some(runtime::write_function);
        config.errorFn = Some(runtime::error_function);

        let WrenBuilder { foreign } = self;
        let user_data = UserData { foreign };
        config.userData = Box::into_raw(Box::new(user_data)) as _;
        config.bindForeignMethodFn = WrenBuilder::foreign_method_bindings();
        config.bindForeignClassFn = WrenBuilder::foreign_class_bindings();

        // WrenVM makes a copy of the configuration. We can
        // discard our copy after creation.
        let vm = unsafe { bindings::wrenNewVM(&mut config) };
        if vm.is_null() {
            panic!("Unexpected null result when creating WrenVM via C");
        }
        WrenVm { vm }
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

/// Utility function for extracting the concrete [`UserData`] instance from
/// the given [`WrenVM`].
unsafe fn get_user_data<'a>(vm: *mut bindings::WrenVM) -> Option<&'a mut UserData> {
    (bindings::wrenGetUserData(vm) as *mut UserData).as_mut()
}

pub struct WrenContext<'wren> {
    pub(crate) vm: &'wren mut bindings::WrenVM,
}

impl<'wren> WrenContext<'wren> {
    pub fn new(vm: &'wren mut bindings::WrenVM) -> Self {
        WrenContext { vm }
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
}

pub struct UserData {
    foreign: ForeignBindings,
}

#[derive(Default)]
pub struct ForeignBindings {
    pub(crate) classes: ForeignClasses,
    pub(crate) methods: ForeignMethods,
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
        let key = (self.module.to_owned(), class.into().into_owned());
        self.foreign.classes.0.insert(key, binding);
    }

    pub fn add_method_binding<S>(&mut self, class: S, binding: ForeignMethod)
    where
        S: Into<Cow<'a, str>>,
    {
        let key = (
            self.module.to_owned(),
            class.into().into_owned(),
            binding.sig.clone(),
        );
        self.foreign.methods.0.insert(key, binding);
    }
}

pub struct ForeignMethod {
    pub is_static: bool,
    pub arity: usize,
    pub sig: String,
    pub func: unsafe extern "C" fn(*mut bindings::WrenVM),
}

#[derive(Default)]
pub struct ForeignMethods(pub HashMap<(String, String, String), ForeignMethod>);

pub struct ForeignClass {
    pub allocate: unsafe extern "C" fn(*mut bindings::WrenVM),
    pub finalize: unsafe extern "C" fn(*mut c_void),
}

#[derive(Default)]
pub struct ForeignClasses(pub HashMap<(String, String), ForeignClass>);

impl ForeignBindings {
    pub fn add_class<M, C>(&mut self, module: M, class: C, binding: ForeignClass)
    where
        M: ToString,
        C: ToString,
    {
        self.classes
            .0
            .insert((module.to_string(), class.to_string()), binding);
    }
}

pub trait WrenClass {
    /// Name of the class as it appears in Wren script.
    const NAME: &'static str;

    fn create(vm: &WrenContext) -> Self;
    fn add_methods() {}
}
