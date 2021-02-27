use rust_wren::{prelude::*, WrenError, WrenResult};

#[wren_class]
#[derive(Debug)]
struct Foo(i32);

#[wren_methods]
impl Foo {
    /// Known value is useful for seeing memory corruption.
    #[construct]
    fn new(val: i32) -> Self {
        Foo(val)
    }

    #[method(name = badReturn)]
    fn bad_return() -> f64 {
        1.0
    }

    #[method(name = badArgs)]
    fn bad_args(&self, _a: f64, _b: bool, _c: String) {
        /* Blank */
    }

    #[method(name = badBorrow)]
    fn bad_borrow(&self, other: &WrenCell<Self>) -> rust_wren::Result<()> {
        // Double borrow when `other` is a reference to `self`.
        let _other = other.try_borrow_mut().map_err(|err| foreign_error!(err))?;
        println!("Other {:?}", _other);

        Ok(())
    }
}

const FOO: &str = r#"
foreign class Foo {
  construct new(val) {}

  foreign static badReturn()
  foreign badArgs(a, b, c)
  foreign badBorrow(other)
  static giveBool() { true }
  static eatme() { Fiber.abort("eatme") }
}
"#;

#[wren_class]
#[derive(Debug)]
struct Bar {
    #[getset]
    baz: f64,
}

#[wren_methods]
impl Bar {
    #[construct]
    fn new() -> Self {
        Self { baz: 0.0 }
    }
}

const BAR: &str = r#"
foreign class Bar {
  construct new() {}

  foreign baz
  foreign baz=(value)
}
"#;

/// Utility for determining if the result is a foreign error from within native Rust inside the Wren VM.
fn is_runtime_foreign_err<T>(result: &WrenResult<T>) -> bool {
    match result {
        Err(err) => match err {
            WrenError::RuntimeError { foreign, .. } => foreign.is_some(),
            _ => false,
        },
        Ok(_) => false,
    }
}

#[test]
fn test_ref_return_type() {
    let mut vm = WrenBuilder::new()
        .with_module("test_error", |module| {
            module.register::<Foo>();
        })
        .build();

    vm.interpret("test_error", FOO).expect("Interpret failed");

    vm.context_result(|ctx| {
        {
            let call_ref = ctx.make_call_ref("test_error", "Foo", "badReturn()")?;

            // ATTENTION: Incorrect return type.
            let result = call_ref.call::<_, bool>(ctx, ());
            assert!(result.is_err());
            println!("{}", result.unwrap_err());
            // match val
        }

        {
            let call_ref = ctx.make_call_ref("test_error", "Foo", "giveBool()")?;
            let result = call_ref.call::<_, String>(ctx, ());
            assert!(result.is_err());
            println!("{}", result.unwrap_err());
        }

        {
            let call_ref = ctx.make_call_ref("test_error", "Foo", "eatme()")?;
            let result = call_ref.call::<_, ()>(ctx, ());
            assert!(result.is_err(), "Result was not an error");

            let error = result.unwrap_err();
            assert!(error.is_runtime_error(), "Result was not a runtime error");
            println!("{}", error);
        }

        Ok(())
    })
    .expect("Context failed");
}

#[test]
fn test_handle_return_type() {
    let mut vm = WrenBuilder::new()
        .with_module("test_error", |module| {
            module.register::<Foo>();
        })
        .build();

    vm.interpret("test_error", FOO).expect("Interpret failed");

    let handle = vm
        .context_result(|ctx| {
            let call_ref = ctx.make_call_ref("test_error", "Foo", "giveBool()")?;

            Ok(call_ref.leak()?)
        })
        .expect("Context failed");

    vm.context_result(|ctx| {
        let result = handle.call::<_, f64>(ctx, ());

        assert!(result.is_err());
        println!("{}", result.unwrap_err());

        Ok(())
    })
    .expect("Context failed");
}

#[test]
fn test_invalid_args() {
    let mut vm = WrenBuilder::new()
        .with_module("test_error", |module| {
            module.register::<Foo>();
        })
        .build();

    vm.interpret("test_error", FOO).expect("Interpret failed");

    // We're testing the functionality that a nice error is
    // returned, and bad foreign function calls don't cause panics.
    let result = vm.interpret(
        "test_error_1",
        r#"
    import "test_error" for Foo
    var foo = Foo.new(0)
    foo.badArgs(true, true, "test")  // first arg is expected to be float
    "#,
    );
    assert!(result.is_err());
    assert!(is_runtime_foreign_err(&result));
    println!("{}", result.unwrap_err());

    // Gracefully detect unacceptable null.
    let result = vm.interpret(
        "test_error_2",
        r#"
    import "test_error" for Foo
    var foo = Foo.new(0)
    foo.badArgs(null, null, null)  // first arg is expected to be float
    "#,
    );
    assert!(result.is_err());
    assert!(is_runtime_foreign_err(&result));
    println!("{}", result.unwrap_err());

    let result = vm.interpret(
        "test_error_3",
        r#"
    import "test_error" for Foo
    var foo = Foo.new(0)
    foo.badBorrow(foo)  // double borrow error
    "#,
    );
    assert!(result.is_err());
    assert!(is_runtime_foreign_err(&result));
    println!("{}", result.unwrap_err());
}

#[test]
fn test_invalid_construct_args() {
    let mut vm = WrenBuilder::new()
        .with_module("test_error", |module| {
            module.register::<Foo>();
        })
        .build();

    vm.interpret("test_error", FOO).expect("Interpret failed");

    let result = vm.interpret(
        "test_error_1",
        r#"
    import "test_error" for Foo
    var foo = Foo.new(null)
    "#,
    );
    assert!(result.is_err());
    assert!(is_runtime_foreign_err(&result));
    println!("{}", result.unwrap_err());
}

#[test]
fn test_prop_type_error() {
    let mut vm = WrenBuilder::new()
        .with_module("test_error", |module| {
            module.register::<Bar>();
        })
        .build();

    vm.interpret("test_error", BAR).expect("Interpret failed");

    let result = vm.interpret(
        "test_error",
        r#"
    var bar = Bar.new()
    bar.baz = null
    "#,
    );

    println!("{}", result.unwrap_err());
}
