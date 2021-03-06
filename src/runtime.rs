/// Callback functions passed to WrenVM.
use crate::{
    bindings,
    errors::WrenVmError,
    vm::WrenVm,
    ForeignError,
};
use smol_str::SmolStr;
use std::{
    alloc::{alloc_zeroed, dealloc, realloc, Layout},
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
    ptr,
};

pub extern "C" fn wren_reallocate(memory: *mut c_void, new_size: usize, _userdata: *mut c_void) -> *mut c_void {
    unsafe {
        if memory.is_null() {
            // Allocate
            alloc_zeroed(Layout::from_size_align(new_size as usize, 8).unwrap()) as *mut _
        } else {
            // Existing memory
            if new_size == 0 {
                // Deallocate
                dealloc(memory as *mut _, Layout::from_size_align(0, 8).unwrap());
                ptr::null_mut()
            } else {
                // Reallocate
                realloc(
                    memory as *mut _,
                    Layout::from_size_align(new_size as usize, 8).unwrap(),
                    new_size as usize,
                ) as *mut _
            }
        }
    }
}

/// Print function backing `System.print()`.
#[no_mangle]
pub extern "C" fn write_function(vm: *mut bindings::WrenVM, text: *const c_char) {
    if let Some(userdata) = unsafe { WrenVm::get_user_data(vm) } {
        let cstr = unsafe { CStr::from_ptr(text) };
        match cstr.to_str() {
            Ok(s) => {
                (userdata.write_fn)(s);
            }
            Err(err) => {
                userdata
                    .errors
                    .borrow_mut()
                    .push(WrenVmError::Foreign(ForeignError::Simple(Box::new(err))));
            }
        }
    }
}

/// Error output.
#[no_mangle]
pub extern "C" fn error_function(
    vm: *mut bindings::WrenVM,
    error_type: bindings::WrenErrorType,
    module: *const c_char,
    line: c_int,
    message: *const c_char,
) {
    if let Some(userdata) = unsafe { WrenVm::get_user_data(vm) } {
        match error_type {
            bindings::WrenErrorType_WREN_ERROR_COMPILE => {
                let c_module = unsafe { CStr::from_ptr(module) };
                let c_message = unsafe { CStr::from_ptr(message) };
                userdata.errors.borrow_mut().push(WrenVmError::Compile {
                    module: SmolStr::new(c_module.to_str().expect("Failed to convert module name to UTF-8")),
                    message: String::from(c_message.to_str().expect("Failed to convert message to UTF-8")),
                    line,
                });
            }
            bindings::WrenErrorType_WREN_ERROR_STACK_TRACE => {
                let c_module = unsafe { CStr::from_ptr(module) };
                let c_message = unsafe { CStr::from_ptr(message) };

                userdata.errors.borrow_mut().push(WrenVmError::StackTrace {
                    module: SmolStr::new(c_module.to_str().expect("Failed to convert module name to UTF-8")),
                    function: SmolStr::from(c_message.to_str().expect("Failed to convert message to UTF-8")),
                    line,
                    is_foreign: false,
                });
            }
            bindings::WrenErrorType_WREN_ERROR_RUNTIME => {
                let c_message = unsafe { CStr::from_ptr(message) };

                userdata.errors.borrow_mut().push(WrenVmError::Runtime {
                    msg: String::from(c_message.to_str().expect("Failed to convert message to UTF-8")),
                });
            }
            _ => {
                unreachable!("Unknown Wren error type: {}", error_type);
            }
        }
    }
}

/// Module resolver
#[no_mangle]
pub extern "C" fn resolve_module(
    vm: *mut bindings::WrenVM,
    importer: *const c_char,
    name: *const c_char,
) -> *const c_char {
    log::trace!("Runtime: resolving module name");

    if let Some(userdata) = unsafe { WrenVm::get_user_data(vm) } {
        let importer = unsafe { CStr::from_ptr(importer) };
        let name = unsafe { CStr::from_ptr(name) };

        if let Some(resolved) = userdata
            .resolver
            .as_mut()
            .and_then(|resolver| resolver.resolve(importer.to_string_lossy().as_ref(), name.to_string_lossy().as_ref()))
        {
            match CString::new(resolved) {
                Ok(c_resolved) => {
                    // Wren takes ownership of the resolved name and deallocates it.
                    c_resolved.into_raw()
                }
                Err(err) => {
                    log::error!("Resolved module name contains a null byte: {}", err);
                    ptr::null()
                }
            }
        } else {
            ptr::null()
        }
    } else {
        ptr::null()
    }
}

/// Module loader
#[no_mangle]
pub extern "C" fn load_module(vm: *mut bindings::WrenVM, name: *const c_char) -> bindings::WrenLoadModuleResult {
    if let Some(userdata) = unsafe { WrenVm::get_user_data(vm) } {
        if let Some(source) = userdata.loader.as_mut().and_then(|loader| {
            let name = unsafe { CStr::from_ptr(name) };
            loader.load(name.to_string_lossy().as_ref())
        }) {
            // Wren takes ownership of the source code string, and deallocates
            // it with `runtime::wren_reallocate`.
            let c_source = CString::new(source).unwrap();
            return bindings::WrenLoadModuleResult {
                source: c_source.into_raw(),
                onComplete: Some(load_module_complete),
                userData: ptr::null_mut(),
            };
        }
    }

    bindings::WrenLoadModuleResult {
        // Null means not found
        source: ptr::null_mut(),
        onComplete: None,
        userData: ptr::null_mut(),
    }
}

/// Callback when module load is done
#[no_mangle]
pub extern "C" fn load_module_complete(
    _vm: *mut bindings::WrenVM,
    _name: *const c_char,
    _result: bindings::WrenLoadModuleResult,
) {
    // TODO: Call module loader on_complete
}
