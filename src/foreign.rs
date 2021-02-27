//! Foreign binding registry.
//!
//! Allows Wren to lookup Rust types at runtime.
use crate::{bindings, WrenVm};
use log::{trace, warn};
use std::{
    any::TypeId,
    collections::HashMap,
    ffi::CStr,
    os::raw::{c_char, c_void},
};

/// Registry of bindings.
pub struct ForeignBindings {
    pub(crate) classes: HashMap<ForeignClassKey, ForeignClass>,
    pub(crate) methods: HashMap<ForeignMethodKey, ForeignMethod>,
    pub(crate) reverse: HashMap<TypeId, ForeignClassKey>,
}

/// Key for foreign class lookup.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ForeignClassKey {
    pub module: String,
    pub class: String,
}

/// Binding to Rust value marked as Wren class.
pub struct ForeignClass {
    pub allocate: unsafe extern "C" fn(*mut bindings::WrenVM),
    pub finalize: unsafe extern "C" fn(*mut c_void),
}

/// Key for foreign method lookup.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ForeignMethodKey {
    pub module: String,
    pub class: String,
    pub sig: String,
    pub is_static: bool,
}

/// Binding to Rust method exposed to Wren.
#[derive(Debug)]
pub struct ForeignMethod {
    pub is_static: bool,
    pub arity: usize,
    pub sig: String,
    pub func: unsafe extern "C" fn(*mut bindings::WrenVM),
}

impl ForeignBindings {
    pub fn new() -> Self {
        ForeignBindings {
            classes: HashMap::new(),
            methods: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    /// Trace print all registered bindings.
    pub fn dump_bindings(&self) {
        trace!("Foreign method bindings");
        for (key, val) in self.methods.iter() {
            trace!("{:?} {:?}", key, val);
        }
    }

    /// Lookup foreign class binding.
    ///
    /// Returns a [`WrenForeignClassMethods`] with None fields if the class is not found.
    pub(crate) extern "C" fn bind_foreign_class(
        vm: *mut bindings::WrenVM,
        module: *const c_char,
        class_name: *const c_char,
    ) -> bindings::WrenForeignClassMethods {
        let userdata = unsafe { WrenVm::get_user_data(vm).expect("User data is null") };

        let module = unsafe { CStr::from_ptr(module).to_owned().to_string_lossy().to_string() };
        let class = unsafe { CStr::from_ptr(class_name).to_owned().to_string_lossy().to_string() };
        trace!("bind_foreign_class {} {}", module, class);

        let (allocate, finalize) = userdata
            .foreign
            .classes
            .get(&ForeignClassKey { module, class })
            .map(|foreign_class| {
                let &ForeignClass { allocate, finalize } = foreign_class;
                (Some(allocate), Some(finalize))
            })
            .unwrap_or_else(|| {
                warn!("Warning: Foreign class not found. Did you forget to register it with the builder?");
                (None, None)
            });

        bindings::WrenForeignClassMethods { allocate, finalize }
    }

    /// Lookup for foreign method binding.
    ///
    /// Returns None if the method is not found.
    pub(crate) extern "C" fn bind_foreign_method(
        vm: *mut bindings::WrenVM,
        module: *const c_char,
        class_name: *const c_char,
        is_static: bool,
        signature: *const c_char,
    ) -> bindings::WrenForeignMethodFn {
        let userdata = unsafe { WrenVm::get_user_data(vm).expect("User data is null") };

        let module = unsafe { CStr::from_ptr(module).to_owned().to_string_lossy().to_string() };
        let class = unsafe { CStr::from_ptr(class_name).to_owned().to_string_lossy().to_string() };
        let sig = unsafe { CStr::from_ptr(signature).to_owned().to_string_lossy().to_string() };

        let key = ForeignMethodKey {
            module,
            class,
            sig,
            is_static,
        };
        trace!("bind_foreign_method {:?}", key);

        let method = userdata.foreign.methods.get(&key).map(|m| m.func);

        if method.is_none() {
            warn!(
                "Warning: Foreign method not found {:?}. Did you forget to register it with the builder?",
                key
            );
        }

        method
    }

    /// Retrieve the module and class name of a registered foreign class.
    pub fn get_class_key<T: 'static>(&self) -> Option<&ForeignClassKey> {
        self.reverse.get(&TypeId::of::<T>())
    }
}

impl Default for ForeignBindings {
    fn default() -> Self {
        Self::new()
    }
}
