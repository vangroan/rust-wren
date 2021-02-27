use rust_wren::{prelude::*, WrenError, WrenResult};
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

/// Retrieving a variable can be of any slot type, including
/// getting foreign values back out of the VM.
///
/// Should be implemented as an API on WrenContext.
#[test]
fn test_variable_foreign_type() {
    #[wren_class]
    #[derive(Debug)]
    struct Foo(u32);

    #[wren_methods]
    impl Foo {
        #[construct]
        fn new(val: u32) -> Self {
            Self(val)
        }
    }

    let mut vm = WrenBuilder::new()
        .with_module("test_context", |module| {
            module.register::<Foo>();
        })
        .build();

    vm.interpret(
        "test_context",
        r#"
    var One = "one"

    class Two {}

    foreign class Foo {
      construct new(val) {}
    }
    var foo = Foo.new(7)
    "#,
    )
    .expect("Interpret failed");

    vm.context(|ctx| {
        use rust_wren::bindings as ffi;
        use std::ffi::CString;

        assert!(ctx.has_module("test_context"));
        assert!(ctx.has_var("test_context", "foo"));

        let c_module = CString::new("test_context").expect("Module name contains a null byte");
        let c_name = CString::new("foo").expect("Name name contains a null byte");

        ctx.ensure_slots(1);
        unsafe {
            ffi::wrenGetVariable(ctx.vm_ptr(), c_module.as_ptr(), c_name.as_ptr(), 0);
            println!("Slot type: {:?}", ctx.slot_type(0));
            let foo_ptr = ffi::wrenGetSlotForeign(ctx.vm_ptr(), 0);
            let foo = (foo_ptr as *mut WrenCell<Foo>).as_mut().unwrap();
            println!("{:?}", foo);
            assert_eq!(foo.borrow().0, 7);
        }
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

    vm.interpret(
        "test_context",
        r#"
    class Foo {
      static bar() {}
      static baz() { 42.0 }
    }
    "#,
    )?;

    vm.context_result(|ctx| {
        let call_ref = ctx.make_call_ref("test_context", "Foo", "bar()")?;
        call_ref.call::<_, ()>(ctx, ()).unwrap();

        Ok(())
    })?;

    let result = vm.context_result(|ctx| {
        let call_ref = ctx.make_call_ref("test_context", "Foo", "baz()")?;
        call_ref.call::<_, f32>(ctx, ())
    })?;

    assert_eq!(result, 42.0);

    Ok(())
}

#[test]
fn test_context_result_fail() -> Result<(), Box<dyn Error>> {
    let mut vm = WrenBuilder::new().build();

    vm.interpret(
        "test_context",
        r#"
    class Foo {
      static bar() {}
    }
    "#,
    )?;

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
