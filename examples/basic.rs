//! Basic usage example of setting up a default VM, interpreting a script,
//! and calling a Wren function from Rust code.
use rust_wren::prelude::*;

fn main() {
    // Build an instance of the Wren VM.
    // It must be mutable to ensure exclusive
    // access when running scripts and
    // mutating VM state.
    //
    // Default builder behaviour sets up
    // Wren's `System.print()` to write
    // to stdout.
    let mut vm: WrenVm = WrenBuilder::new().build();

    // Scripts are any UTF-8 formatted string
    // that doesn't contain a null byte.
    //
    // They can be constant or loaded from a file,
    // and safely dropped after compilation.
    let script: &str = r#"
    System.print("Hello, Rust! 🦀")
    "#;

    // Script string is copied to the VM's
    // heap memory by Wren's compiler.
    //
    // Interpreting a script runs it in a new
    // fiber in the context of the resolved module.
    let result = vm.interpret("my_module", script);

    // Errors in the compiler during parsing, or in the script
    // during execution, are collected and returned.
    match result {
        Err(rust_wren::WrenError::CompileError(errors)) => {
            // When the compiler encounters syntax errors,
            // it keeps consuming tokens, building a list
            // of multiple parsing errors.
            for e in &errors {
                eprintln!("[{} line {}] {}", e.module, e.line, e.message);
            }
        }
        Err(rust_wren::WrenError::RuntimeError { message, stack, .. }) => {
            // Runtime errors are anything that aborts
            // the fiber during execution. This can be
            // from out-of-bounds access, or from `Fiber.abort()`.
            eprintln!("{}", message);

            // Stacktrace is ordered from top to bottom.
            // The current function where the runtime error
            // occurred will be first.
            for frame in &stack {
                eprintln!("[{} line {}] in {}", frame.module, frame.line, frame.function);
            }
        }
        _ => {}
    }

    // Wren functions can be called from Rust.
    //
    // The API for calling Wren is accessed via a contextual
    // closure. This is because obtaining handles to Wren values
    // prevents them from being garbage collected. All handles
    // must be released before the VM is dropped.
    //
    // The closure also helps guarantee exclusive access to
    // the VM instance by requiring a mutable reference.
    // Scripts cannot be interpreted while the closure is executing
    // because the closure cannot borrow the VM.
    let result = vm.context_result(|ctx| {
        // Wren functions are called via a call handle, which
        // contains two Wren handles. One to a receiver value,
        // and one to a compiled function.
        //
        // To build a call handle, first we need to lookup
        // a variable in a module to use as a receiver.
        // All calls must have a receiver, and for
        // static functions such as `print` the receiver
        // is the class.
        //
        // Because we interpreted a script in the `my_module`
        // module earlier, the built-in `System` class is now
        // available as a top-level variable via that module.
        //
        // Lastly we need a function signature, which is
        // the function name and arguments as underscores.
        let print_func = ctx.make_call_ref("my_module", "System", "print(_)")?;

        // The call handle is just that, a handle to a function
        // inside the Wren VM.
        //
        // To perform the actual call, we must pass in the
        // VM context. The arguments to the call can be
        // any type that implements the trait `ToWren`.
        //
        // The return type is specified by the type argument
        // to the `WrenCallRef::call` function.
        print_func.call::<_, ()>(ctx, "Hello, Wren! 🐦")?;

        // The lifetime of the `WrenCallRef` is tied to this
        // closure and cannot escape it. It is dropped at
        // the end of this scope, releasing the receiver
        // and function handles, allowing them to be garbage
        // collected in Wren later.

        // Values can be returned from the context closure.
        Ok(())
    });

    // Like `WrenVM::interpret()`, calling functions using
    // the context can result in runtime errors.
    result.expect("calling System.print() failed");
}
