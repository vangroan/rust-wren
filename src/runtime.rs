/// Callback functions passed to WrenVM.
use crate::bindings;
use std::{
    ffi::CStr,
    os::raw::{c_char, c_int},
};

/// Print function backing `System.print()`.
#[no_mangle]
pub extern "C" fn write_function(_vm: *mut bindings::WrenVM, text: *const c_char) {
    let cstr = unsafe { CStr::from_ptr(text) };
    match cstr.to_str() {
        Ok(s) => {
            print!("{}", s);
        }
        Err(err) => {
            eprint!("{}", err);
        }
    }
}

/// Error output.
#[no_mangle]
pub extern "C" fn error_function(
    _vm: *mut bindings::WrenVM,
    error_type: bindings::WrenErrorType,
    module: *const c_char,
    line: c_int,
    message: *const c_char,
) {
    match error_type {
        bindings::WrenErrorType_WREN_ERROR_COMPILE => {
            let c_module = unsafe { CStr::from_ptr(module) };
            let c_message = unsafe { CStr::from_ptr(message) };
            eprintln!(
                "[{} line {}] [Error] {}",
                c_module
                    .to_str()
                    .expect("Failed to convert module name to UTF-8"),
                line,
                c_message
                    .to_str()
                    .expect("Failed to convert message to UTF-8")
            );
        }
        bindings::WrenErrorType_WREN_ERROR_STACK_TRACE => {
            let c_module = unsafe { CStr::from_ptr(module) };
            let c_message = unsafe { CStr::from_ptr(message) };
            eprintln!(
                "[{} line {}] [Error] in {}",
                c_module
                    .to_str()
                    .expect("Failed to convert module name to UTF-8"),
                line,
                c_message
                    .to_str()
                    .expect("Failed to convert message to UTF-8")
            );
        }
        bindings::WrenErrorType_WREN_ERROR_RUNTIME => {
            let c_message = unsafe { CStr::from_ptr(message) };
            eprintln!(
                "[Runtime Error] {}",
                c_message
                    .to_str()
                    .expect("Failed to convert message to UTF-8")
            );
        }
        _ => {
            panic!("Unknown Wren error type: {}", error_type);
        }
    }
}
