use rust_wren::prelude::*;
use rust_wren::{WrenCompileError, WrenError, WrenStackFrame};

#[derive(Debug)]
struct SomeError;

impl std::fmt::Display for SomeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "some test error")
    }
}

impl std::error::Error for SomeError {}

mod foo {
    use super::*;

    #[wren_class]
    pub struct Foo;

    #[wren_methods]
    impl Foo {
        #[construct]
        pub fn new() -> Self {
            Self
        }

        pub fn eatme() -> rust_wren::Result<()> {
            // let _f = fs::File::open("unknown.txt")?;

            Err(foreign_error!(SomeError))
        }
    }
}

#[test]
fn test_compile_error() {
    use foo::*;
    let mut vm = WrenBuilder::new()
        .with_module("depend", |module| module.register::<Foo>())
        .build();

    vm.interpret(
        "depend",
        r#"
    foreign class Foo {
        construct new() {}
        foreign static eatme()
    }

    class Failer {
        static eatMe() { Foo.eatme() }
        static eatMe1(value) { internal_() }
        static internal_() { Fiber.abort("fubar") }
    }
    "#,
    )
    .expect("Interpret failed");

    let result = vm.interpret(
        "test_compile_error",
        r#"
    import "depend" for Failer, Foo
    // Failer.eatMe1("qwerty")
    // Fiber.abort("failed on purpose")
    Failer.eatMe()

    // class 123EatMe {
    //    construct new() {}
    //    static some() {}
    // }
    "#,
    );

    if let Err(err) = &result {
        eprintln!("{}", err);
    }


    match result {
        Err(WrenError::CompileError(errors)) => {
            for WrenCompileError { module, message, line } in errors {
                eprintln!("[Rust Result] Compile Error [{} line {}] {}", module, line, message);
            }
        }
        Err(WrenError::RuntimeError {
            message,
            foreign,
            stack,
        }) => {
            let mut msg = String::new();

            if let Some(err) = foreign {
                msg.push_str(&format!("[Rust Result] Foreign Runtime Error: {}\n", err));
            } else {
                msg.push_str(&format!("[Rust Result] Script Runtime Error: {}\n", message));
            };

            msg.push_str("Stack Trace:\n");

            let count = stack.len();
            for (idx, frame) in stack.into_iter().enumerate() {
                let WrenStackFrame { module, line, function, is_foreign } = frame;

                if is_foreign {
                    msg.push_str(&format!("\t{}. *foreign {}:{}\n", count - idx, module, line,));
                } else {
                    msg.push_str(&format!("\t{}. {} {}:{}\n", count - idx, module, function, line,));
                }
            }

            eprintln!("{}", msg);
        }
        _ => {}
    }
}
