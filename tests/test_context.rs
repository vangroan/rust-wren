use rust_wren::{prelude::*, WrenResult, WrenError};
use std::{cell::RefCell, error::Error};

/// Should check whether a variable exists or not.
#[test]
fn test_has_variable() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret(
        "test_context",
        r#"
    var One = "one"

    class Two {}
    "#,
    )
    .expect("Interpret failed");

    vm.context(|ctx| {
        assert!(ctx.has_var("test_context", "One"));
        assert!(ctx.has_var("test_context", "Two"));
        assert!(!ctx.has_var("test_context", "Three"));
        assert!(!ctx.has_var("unknown", "One"));
    });
}

#[test]
fn test_has_module() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret("test_context", "").expect("Interpret failed");

    vm.context(|ctx| {
        assert!(ctx.has_module("test_context"));
        assert!(!ctx.has_module("unknown"));
    });
}

#[test]
fn test_write_fn() {
    thread_local! {static CALL_COUNT: RefCell<usize> = RefCell::new(0); }

    let mut vm = WrenBuilder::new()
        .with_write_fn(|s| {
            CALL_COUNT.with(|f| {
                *f.borrow_mut() += 1;
            });
            print!("{}", s);
        })
        .build();

    vm.interpret(
        "test_context",
        r#"
    System.print("a")
    System.print("b")
    System.print("c")
    "#,
    )
    .expect("Interpret failed");

    CALL_COUNT.with(|f| {
        // Wren prints a new line as a separate call, so number of calls are doubled.
        assert_eq!(*f.borrow(), 6);
    });
}

#[test]
fn test_context_result() -> WrenResult<()> {
    let mut vm = WrenBuilder::new().build();

    vm.interpret("test_context", r#"
    class Foo {
      static bar() {}
    }
    "#)?;

    vm.context_result(|ctx| {
        let call_ref = ctx.make_call_ref("test_context", "Foo", "bar()")?;
        call_ref.call::<_, ()>(ctx, ()).unwrap();

        Ok(())
    })?;

    Ok(())
}

#[test]
fn test_context_result_fail() -> Result<(), Box<dyn Error>> {
    let mut vm = WrenBuilder::new().build();

    vm.interpret("test_context", r#"
    class Foo {
      static bar() {}
    }
    "#)?;

    let result = vm.context_result(|ctx| {
        let call_ref = ctx.make_call_ref("test_context", "Undefined", "bar()")?;
        call_ref.call::<_, ()>(ctx, ()).unwrap();

        Ok(())
    });

    // Negative test; flip result.
    match result {
        Ok(_) => Err(format!("Unexpected success").into()),
        Err(err) => {
            if matches!(err, WrenError::VariableNotFound(_)) {
                Ok(())
            } else {
                Err(format!("Unexpected error returned: {}", err).into())
            }
        }
    }
}
