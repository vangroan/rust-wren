use crate::{
    bindings,
    value::ToWren,
    vm::{WrenContext, WrenVm},
};
use smol_str::SmolStr;
use std::{
    ffi::CString,
    fmt::{self, Display},
};

pub type WrenResult<T> = ::std::result::Result<T, WrenError>;

#[derive(Debug)]
pub enum WrenError {
    CompileError(Vec<WrenCompileError>),
    RuntimeError {
        message: String,
        foreign: Option<Box<dyn ::std::error::Error>>,
        stack: Vec<WrenStackFrame>,
    },
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
        }
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
            bindings::wrenSetSlotString(ctx.vm, slot, c_string.as_ptr());
            bindings::wrenAbortFiber(ctx.vm, slot);
        }

        // Send a stack frame to the error channel so the printed stack trace
        // can show the failure in the foreign function.
        //
        // Wren doesn't trace the call to the foreign function.
        if let Some(userdata) = unsafe { WrenVm::get_user_data(ctx.vm) } {
            // TODO: Does the order of these two sends need to be reversed?
            if let ForeignError::Annotated { line, module, .. } = &self {
                userdata
                    .error_tx
                    .send(WrenVmError::StackTrace {
                        module: module.clone().into(),
                        function: "".into(),
                        line: *line,
                        is_foreign: true,
                    })
                    .expect("Failed to send stack frame to error channel");
            }

            userdata
                .error_tx
                .send(WrenVmError::Foreign(self))
                .expect("Failed to send foreign error to error channel");
        }
    }
}
