use crate::{bindings, runtime};
use std::{
    ffi::CString,
    {fmt, ptr},
};

pub struct WrenVm {
    vm: *mut bindings::WrenVM,
}

impl WrenVm {
    pub fn interpret(self, module: &str, source: &str) -> WrenResult<()> {
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
}

impl Drop for WrenVm {
    fn drop(&mut self) {
        if !self.vm.is_null() {
            unsafe { bindings::wrenFreeVM(self.vm) };
            // VM is now deallocated.
            self.vm = ptr::null_mut();
        }
    }
}

pub struct WrenBuilder {}

impl WrenBuilder {
    pub fn new() -> WrenBuilder {
        Default::default()
    }

    pub fn build(self) -> WrenVm {
        let mut config = bindings::WrenConfiguration {
            reallocateFn: None,
            resolveModuleFn: None,
            loadModuleFn: None,
            bindForeignMethodFn: None,
            bindForeignClassFn: None,
            writeFn: None,
            errorFn: None,
            initialHeapSize: 0,
            minHeapSize: 0,
            heapGrowthPercent: 0,
            userData: ptr::null_mut(),
        };

        // Apply default heap settings.
        unsafe { bindings::wrenInitConfiguration(&mut config) };

        config.writeFn = Some(runtime::write_function);
        config.errorFn = Some(runtime::error_function);

        // WrenVM makes a copy of the configuration. We can
        // discard our copy after creation.
        let vm = unsafe { bindings::wrenNewVM(&mut config) };
        if vm.is_null() {
            panic!("Unexpected null result when creating WrenVM via C");
        }
        WrenVm { vm }
    }
}

impl Default for WrenBuilder {
    fn default() -> WrenBuilder {
        WrenBuilder {}
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
