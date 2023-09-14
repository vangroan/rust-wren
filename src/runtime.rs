/// Callback functions passed to WrenVM.
use crate::{bindings, errors::WrenVmError, vm::WrenVm, ForeignError};
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
            if new_size == 0 {
                // Called by Wren when an empty list is cleared.
                // Wren assume malloc semantics where size 0 does
                // nothing and returns NULL.
                ptr::null_mut()
            } else {
                // Allocate
                record_alloc(
                    alloc_zeroed(Layout::from_size_align(new_size as usize, 8).unwrap()) as *mut _,
                    new_size,
                    1,
                )
            }
        } else {
            // Existing memory
            if new_size == 0 {
                // Deallocate
                dealloc(memory as *mut _, Layout::from_size_align(0, 8).unwrap());
                record_alloc(memory, 0, -1);
                ptr::null_mut()
            } else {
                // Reallocate
                record_alloc(memory, 0, -1);
                // Rust realloc returns a new address if ownsership of
                // the block has changed, or null when ownsership cannot be taken.
                record_alloc(
                    realloc(
                        memory as *mut _,
                        Layout::from_size_align(new_size as usize, 8).unwrap(),
                        new_size as usize,
                    ) as *mut _,
                    new_size,
                    1,
                )
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
            // Length in bytes, not chars or graphmemes.
            let source_len = source.len();

            // Wren takes ownership of the source code string, then
            // passes it back by calling `load_module_complete`.
            let c_source = CString::new(source).unwrap();
            let source = c_source.into_raw();

            // Bookkeeping for string allocation, as we are responsible
            // for safely deallocating this string when Wren is done compiling.
            unsafe {
                record_alloc(source as *mut _, source_len, 1);
            }

            return bindings::WrenLoadModuleResult {
                source,
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
    vm: *mut bindings::WrenVM,
    name: *const c_char,
    result: bindings::WrenLoadModuleResult,
) {
    // Deallocate source string.
    let bindings::WrenLoadModuleResult { source, .. } = result;

    if !source.is_null() {
        unsafe {
            record_alloc(source as *mut _, 0, -1);
        }

        // Convert to Rust string and trigger drop.
        // `from_raw` should only ever be called with a pointer created by `into_raw`.
        let source = unsafe { CString::from_raw(source as *mut _) };
        drop(source);
    }

    // Call module loader on_complete
    if let Some(userdata) = unsafe { WrenVm::get_user_data(vm) } {
        if let Some(ref mut loader) = userdata.loader {
            let name = unsafe { CStr::from_ptr(name) };
            loader.on_complete(name.to_string_lossy().as_ref());
        }
    }
}

#[cfg(debug_assertions)]
mod alloc_debug {
    use std::{collections::HashMap, sync::RwLock};

    #[derive(Default)]
    pub(crate) struct AllocRecord {
        pub(crate) count: i64,
        /// Last allocation size.
        pub(crate) size: usize,
    }

    lazy_static! {
        // Record of allocations, leaks and double frees.
        // Maps memory address to number of allocation calls.
        // The allocation count must be either 0 or 1.
        // When the count exceeds 1, it means the same address was allocated
        // multiple times. When it's -1 or lower, multiple frees took place.
        pub(crate) static ref ALLOCS: RwLock<HashMap<usize, AllocRecord>> = RwLock::new(HashMap::new());
    }
}

/// Enter an allocation record into the global registry,
/// prints out warnings when improper reallocations are detected.
///
/// # Safety
///
/// Doesn't do anything unsafe with the given pointer, but
/// marked unsafe because it takes a raw pointer.
#[inline]
#[allow(unused_variables)]
unsafe fn record_alloc(address: *mut c_void, size: usize, diff: i64) -> *mut c_void {
    #[cfg(debug_assertions)]
    {
        use log::warn;

        let key = address as usize;

        if let Ok(mut allocs) = alloc_debug::ALLOCS.write() {
            let record = allocs.entry(key).or_insert_with(Default::default);
            record.count += diff;

            // Keep last requested size to assist with debugging.
            // Don't overwrite last size when deallocating.
            if size != 0 {
                record.size = size;
            }

            if record.count > 1 {
                warn!(
                    "alloc: address {:?} allocated {} times, last size {}",
                    address, record.count, record.size
                );
            } else if record.count < 0 {
                warn!(
                    "alloc: address {:?} deallocated {} times, last size {}",
                    address,
                    record.count.abs(),
                    record.size
                );
            } else if record.count == 0 {
                // When properly deallocated, remove from map
                // so we don't cause leaks ourselves.
                allocs.remove(&key);
            }
        }
    }
    // Pass the address through so allocation calls
    // can be wrapped in this function.
    // In a release build this function will be inlined away.
    return address;
}

/// Assert that all Wren's heap memory has been deallocated.
///
/// Requires `debug_assertions`, otherwise does nothing.
///
/// # Panic
///
/// Panics when there are allocations left in the debug registry.
pub fn assert_all_deallocated() {
    #[cfg(debug_assertions)]
    {
        use log::{info, warn};

        let allocs = alloc_debug::ALLOCS.read().expect("unlocking allocation registry");
        if !allocs.is_empty() {
            for (address, record) in allocs.iter() {
                warn!(
                    "alloc: address {:?} allocated {} times, last size {}",
                    *address as *mut u8, record.count, record.size
                );
            }
            panic!("Leaked {} allocations. See previous logs for details", allocs.len());
        } else {
            info!("alloc: no allocations on heap");
        }
    }
}

/// Print current allocation registry to logs.
///
/// Requires `debug_assertions`, otherwise does nothing.
pub fn dump_allocations() {
    #[cfg(debug_assertions)]
    {
        use log::info;

        if let Ok(allocs) = alloc_debug::ALLOCS.read() {
            for (address, record) in allocs.iter() {
                info!(
                    "alloc: address {:?} allocated {} times, last size {}",
                    *address as *mut u8, record.count, record.size
                );
            }
        }
    }
}
