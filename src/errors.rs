use crate::{
    bindings,
    types::WrenType,
    value::ToWren,
    vm::{WrenContext, WrenVm},
};
use smol_str::SmolStr;
use std::{
    error::Error,
    ffi::CString,
    fmt::{self, Display},
};

pub type WrenResult<T> = ::std::result::Result<T, WrenError>;

#[derive(Debug)]
pub enum WrenError {
    CompileError(Vec<WrenCompileError>),
    RuntimeError {
        message: String,
        foreign: Option<Box<dyn Error>>,
        stack: Vec<WrenStackFrame>,
    },
    ModuleNotFound(String),
    VariableNotFound(String),
    ResultQueueMismatch,
    ErrorAbsent(bindings::WrenInterpretResult),
    UserDataNull,
    SizeMismatch,

    /// Error when Wren provides a pointer which is unexpectedly null.
    ///
    /// This most likely indicates a bug in either Wren o r `rust-wren`.
    NullPtr,
    InvalidSlot,
    SlotOutOfBounds(i32),
    SlotType {
        expected: WrenType,
        actual: WrenType,
    },
    Utf8(::std::str::Utf8Error),
    ForeignType,

    /// Wrapped error caused by invalid call from Wren to Rust.
    /// Used in generated code of wrapped functions.
    ForeignCall {
        function: SmolStr,
        cause: Box<WrenError>,
    },

    /// Wrapped error when getting a function call argument from
    /// a slot, sent from Wren, fails.
    /// Used in generated code of wrapped functions.
    GetArg {
        slot: i32,
        cause: Box<WrenError>,
    },

    /// When leaking a [`WrenRef`](handle/struct.WrenRef.html), ownership of the destructor channel sender must
    /// be moved to the leaked handle.
    ///
    /// If the existing sender has already been moved, then it means the handle
    /// is being leaked twice.
    AlreadyLeaked,

    /// Attempt to borrow `WrenCell`, but already borrowed.
    BorrowMutError,

    /// Attempt to borrow `WrenCell`, but already borrowed.
    BorrowError,

    /// Wrapper for errors that occur within a context closure.
    Ctx(Box<dyn Error>),
}

impl ::std::error::Error for WrenError {}

impl ::std::fmt::Display for WrenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Useful error information is output by Wren using the `errorFn`.
        match self {
            WrenError::CompileError(errors) => {
                for err in errors {
                    writeln!(f, "[{} line {}] [Error] {}", err.module, err.line, err.message)?;
                }
                Ok(())
            }
            WrenError::RuntimeError { message, stack, .. } => {
                writeln!(f, "[Runtime Error] {}", message)?;

                for frame in stack {
                    let is_foreign = if frame.is_foreign { "*" } else { "" };
                    writeln!(
                        f,
                        "[{}{} line {}] [Error] in {}",
                        is_foreign, frame.module, frame.line, frame.function
                    )?;
                }

                Ok(())
            }
            WrenError::ModuleNotFound(mod_name) => write!(f, "Module '{}' not found", mod_name),
            WrenError::VariableNotFound(var_name) => write!(f, "Variable '{}' not found", var_name),
            WrenError::ResultQueueMismatch => write!(
                f,
                "Wren VM returned success, but errors were recorded on the error queue"
            ),
            WrenError::ErrorAbsent(result_id) => write!(
                f,
                "Wren VM failed with result {}, but no errors were recorded on the error queue",
                result_id
            ),
            WrenError::UserDataNull => write!(f, "User data pointer in VM is null"),
            WrenError::SizeMismatch => write!(f, "List size and slie size must be equal"),
            WrenError::NullPtr => writeln!(f, "Unexpected null pointer"),
            WrenError::SlotOutOfBounds(slot) => write!(f, "Slot {} is out of bounds", slot),
            WrenError::SlotType { expected, actual } => {
                write!(f, "Expected slot type '{:?}', actual '{:?}'", expected, actual)
            }
            WrenError::InvalidSlot => write!(f, "Invalid slot"),
            WrenError::Utf8(utf8_err) => ::std::fmt::Display::fmt(utf8_err, f),
            WrenError::ForeignType => write!(f, "Unexpected foreign type"),
            WrenError::ForeignCall { function, cause } => {
                write!(f, "Invalid call to foreign '{}': {}", function, cause)
            }
            WrenError::GetArg { slot, cause } => write!(f, "Getting argument from slot {} failed: {}", slot, cause),
            WrenError::AlreadyLeaked => write!(f, "Already leaked handle"),
            WrenError::BorrowMutError | WrenError::BorrowError => write!(
                f,
                "Foreign class already borrowed. Was it passed into multiple foreign call arguments?"
            ),
            WrenError::Ctx(err) => write!(f, "Error in Wren context closure: {}", err),
        }
    }
}

impl WrenError {
    /// Construct a `ForeignCall` variant.
    ///
    /// Intended to be used by generated code that wraps
    /// Rust fucntions and exposes them to Wren.
    #[inline]
    #[doc(hidden)]
    pub fn new_foreign_call<S>(function_name: S, cause: Box<WrenError>) -> Self
    where
        S: AsRef<str>,
    {
        WrenError::ForeignCall {
            function: SmolStr::new(function_name),
            cause,
        }
    }

    #[inline]
    pub fn is_runtime_error(&self) -> bool {
        matches!(self, WrenError::RuntimeError { .. })
    }

    #[inline]
    pub fn is_compile_error(&self) -> bool {
        matches!(self, WrenError::CompileError(_))
    }
}

/// Wren VM errors collected from the error callback function.
///
/// This is not the error type return by the VM result. You may be looking for [`WrenError`](enum.WrenError.html).
#[derive(Debug)]
pub enum WrenVmError {
    /// Error in Wren VM compiling script.
    Compile {
        module: SmolStr,
        message: String,
        line: i32,
    },
    /// Error in Wren VM during runtime.
    Runtime { msg: String },
    /// Stack trace frame.
    StackTrace {
        module: SmolStr,
        function: SmolStr,
        line: i32,
        is_foreign: bool,
    },
    /// Runtime error from a foreign function, called from `wrenInterpret` or `wrenCall`.
    Foreign(ForeignError),
}

#[derive(Debug)]
pub struct WrenStackFrame {
    pub module: SmolStr,
    pub function: SmolStr,
    pub line: i32,
    pub is_foreign: bool,
}

/// Wren compilation error.
///
/// Wren's compiler continues on encountering a syntax error. These
/// errors all need to be collected and sent to the user.
#[derive(Debug)]
pub struct WrenCompileError {
    pub module: SmolStr,
    pub message: String,
    pub line: i32,
}

pub type Result<T> = ::std::result::Result<T, ForeignError>;

impl<T> ToWren for self::Result<T>
where
    T: ToWren,
{
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        match self {
            Ok(val) => val.put(ctx, slot),
            Err(err) => err.put(ctx, slot),
        }
    }
}

/// Error for use by foreign methods.
#[derive(Debug)]
pub enum ForeignError {
    Simple(Box<dyn ::std::error::Error>),
    Annotated {
        line: i32,
        module: String,
        inner: Box<dyn ::std::error::Error>,
    },
}

impl ForeignError {
    pub fn new<T: ::std::error::Error + 'static>(inner: T) -> Self {
        ForeignError::Simple(Box::new(inner))
    }

    pub fn inner(&self) -> &(dyn ::std::error::Error + 'static) {
        match self {
            ForeignError::Simple(inner) => &**inner,
            ForeignError::Annotated { inner, .. } => &**inner,
        }
    }

    pub fn take_inner(self) -> Box<dyn ::std::error::Error> {
        match self {
            ForeignError::Simple(inner) => inner,
            ForeignError::Annotated { inner, .. } => inner,
        }
    }
}

impl Display for ForeignError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self.inner(), f)
    }
}

impl ::std::error::Error for ForeignError {
    fn source(&self) -> Option<&(dyn ::std::error::Error + 'static)> {
        Some(self.inner())
    }
}

/// Spicy implementation that also aborts the current fiber.
impl ToWren for ForeignError {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        let c_string = CString::new(format!("{}", self.inner())).expect("String contains a null byte");
        unsafe {
            bindings::wrenSetSlotString(ctx.vm_ptr(), slot, c_string.as_ptr());
            bindings::wrenAbortFiber(ctx.vm_ptr(), slot);
        }

        // Send a stack frame to the error channel so the printed stack trace
        // can show the failure in the foreign function.
        //
        // Wren doesn't trace the call to the foreign function.
        if let Some(userdata) = unsafe { WrenVm::get_user_data(ctx.vm_ptr()) } {
            if let ForeignError::Annotated { line, module, .. } = &self {
                userdata.errors.borrow_mut().push(WrenVmError::StackTrace {
                    module: module.clone().into(),
                    function: "(foreign)".into(),
                    line: *line,
                    is_foreign: true,
                });
            }

            userdata.errors.borrow_mut().push(WrenVmError::Foreign(self));
        }
    }
}
