use rust_wren::{
    handle::{FnSymbolRef, WrenCallHandle, WrenCallRef, WrenHandle},
    prelude::*,
};
use std::{rc::Rc, thread};

const CLASS: &str = r#"
class TestHandle {
    static print() {
        System.print("Wren: TestHandle.print() called")
    }

    static withArgs(one, two, three) { one + two + three }
}
"#;

#[wren_class]
#[derive(Debug)]
struct MoveMe(f64);

#[wren_methods]
impl MoveMe {
    #[construct]
    fn new(inner: f64) -> Self {
        Self(inner)
    }

    fn inner(&self) -> f64 {
        println!("Rust: MoveMe.inner() -> {}", self.0);
        self.0
    }

    fn one(&self, first: f64) -> f64 {
        let result = self.0 + first;
        println!("Rust: MoveMe.one({}) -> {}", first, result);
        result
    }

    fn two(&self, first: f64, second: f64) -> f64 {
        let result = self.0 + first + second;
        println!("Rust: MoveMe.two({}, {}) -> {}", first, second, result);
        result
    }
}

const MOVE_ME: &str = r#"
foreign class MoveMe {
    construct new(inner) {}
    foreign inner()
    foreign one(first)
    foreign two(first, second)
}
"#;

/// Should call method of Wren class.
#[test]
fn test_wren_call() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret("test_handle", CLASS).unwrap();

    vm.context(|ctx| {
        // Static call looks up class declaration as variable.
        let test_class = ctx.get_var("test_handle", "TestHandle").unwrap();
        let print_fn = FnSymbolRef::compile(ctx, "print()").unwrap();
        let call_handle = WrenCallRef::new(test_class, print_fn);

        println!("Rust: Calling TestHandle.print()");
        call_handle.call::<_, ()>(ctx, ());
    });

    vm.context(|ctx| {
        // Static call looks up class declaration as variable.
        let test_class = ctx.get_var("test_handle", "TestHandle").unwrap();
        let print_fn = FnSymbolRef::compile(ctx, "withArgs(_,_,_)").unwrap();
        let call_handle = WrenCallRef::new(test_class, print_fn);

        println!("Rust: Calling TestHandle.withArgs(_,_,_)");
        assert_eq!(call_handle.call::<_, f64>(ctx, (3.0, 7.0, 11.0)), Some(21.0));
    });
}

/// Should call method on foreign class.
#[test]
fn test_foreign_call() {
    let mut vm = WrenBuilder::new()
        .with_module("test_handle", |module| module.register::<MoveMe>())
        .build();

    vm.interpret("test_handle", MOVE_ME).unwrap();
    vm.interpret("test_handle", r#"var m = MoveMe.new(7)"#).unwrap();

    vm.context(|ctx| {
        // Instance method
        let move_me_obj = ctx.get_var("test_handle", "m").unwrap();
        let func = FnSymbolRef::compile(ctx, "inner()").unwrap();
        let call_handle = WrenCallRef::new(move_me_obj, func);

        println!("Rust: Calling MoveMe.inner()");
        call_handle.call::<_, ()>(ctx, ());
    });
}

/// Should call methods with multiple arguments
#[test]
fn test_call_handle_with_arguments() {
    let mut vm = WrenBuilder::new()
        .with_module("test_handle", |module| module.register::<MoveMe>())
        .build();

    vm.interpret("test_handle", MOVE_ME).unwrap();
    vm.interpret("test_handle", r#"var m = MoveMe.new(11)"#).unwrap();

    vm.context(|ctx| {
        // Instance method
        let move_me_obj = ctx.get_var("test_handle", "m").unwrap();
        let func = FnSymbolRef::compile(ctx, "one(_)").unwrap();
        let call_handle = WrenCallRef::new(move_me_obj, func);

        println!("Rust: Calling MoveMe.one(_)");
        let result: f64 = call_handle.call::<f64, f64>(ctx, 7.0).unwrap();
        assert_eq!(result, 18.0);
    });

    vm.context(|ctx| {
        // Instance method
        let move_me_obj = ctx.get_var("test_handle", "m").unwrap();
        let func = FnSymbolRef::compile(ctx, "two(_,_)").unwrap();
        let call_handle = WrenCallRef::new(move_me_obj, func);

        println!("Rust: Calling MoveMe.two(_,_)");
        let result: f64 = call_handle.call::<_, f64>(ctx, (7.0, 3.0)).unwrap();
        assert_eq!(result, 21.0);
    });
}

/// Check that WrenRef can be passed multiple times.
#[test]
fn test_multiple_arg_passes() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret(
        "test_handle",
        r#"
    class Test {
        static calc(val) { val * val }
    }

    var a = 4
    "#,
    )
    .expect("Interpret failed");

    vm.context(|ctx| {
        let call_ref = ctx.make_call_ref("test_handle", "Test", "calc(_)").unwrap();
        let arg_a = ctx.get_var("test_handle", "a").unwrap();

        call_ref.call::<_, f64>(ctx, &arg_a).unwrap();
        call_ref.call::<_, f64>(ctx, &arg_a).unwrap();
        call_ref.call::<_, f64>(ctx, &arg_a).unwrap();
    });
}

#[test]
fn test_non_existing() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret("test_handle", "").unwrap();

    vm.context(|ctx| {
        assert!(ctx.get_var("test_handle", "m").is_err());
        assert!(ctx.get_var("unknown", "m").is_err());
    });
}

/// Check that a fiber is passed as a WrenHandle.
#[test]
fn test_fiber_handle() {
    #[wren_class]
    struct CallMe;

    #[wren_methods]
    impl CallMe {
        #[construct]
        fn new() -> Self {
            CallMe
        }

        fn call(fiber: WrenRef<'_>) {
            println!("Got handle {:?}", fiber);
        }
    }
    let mut vm = WrenBuilder::new()
        .with_module("test_handle", |module| {
            module.register::<CallMe>();
        })
        .build();

    vm.interpret(
        "test_handle",
        r#"
    foreign class CallMe {
        construct new() {}
        foreign static call(fiber)
    }

    CallMe.call(Fiber.current)
    "#,
    )
    .unwrap();
}

#[test]
fn test_wren_ref_leak() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret(
        "test_handle",
        r#"
    class Test {
        static unwrap(val) { val }
    }

    var a = "Foo"
    var b = "Bar"
    "#,
    )
    .expect("Interpret failed");

    let mut handle: Option<WrenHandle> = None;
    vm.context(|ctx| {
        let a_ref = ctx.get_var("test_handle", "a").unwrap();
        handle = a_ref.leak();
    });

    assert!(handle.is_some());

    vm.context(|ctx| {
        let unwrap_fn = ctx.make_call_ref("test_handle", "Test", "unwrap(_)").unwrap();
        let unwrapped_a = unwrap_fn.call::<_, String>(ctx, handle).unwrap(); // <-- handle drop

        assert_eq!(unwrapped_a.as_str(), "Foo");
    });

    let mut rc: Option<Rc<WrenHandle>> = None;
    vm.context(|ctx| {
        let b_ref = ctx.get_var("test_handle", "b").unwrap();
        rc = b_ref.leak().map(|r| Rc::new(r));
    });

    // This should be dropped by WrenVm::drop
    let _rc_cloned = rc.clone();

    vm.context(|ctx| {
        let unwrap_fn = ctx.make_call_ref("test_handle", "Test", "unwrap(_)").unwrap();

        // First call
        let unwrapped_b1 = unwrap_fn.call::<_, String>(ctx, rc.clone()).unwrap();
        assert_eq!(unwrapped_b1.as_str(), "Bar");

        // Second call
        let unwrapped_b2 = unwrap_fn.call::<_, String>(ctx, rc).unwrap(); // <-- rc drop
        assert_eq!(unwrapped_b2.as_str(), "Bar");
    });
}

#[test]
fn test_wren_call_ref_leak() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret(
        "test_handle",
        r#"
    class Test {
        static calc(val) { val * val }
    }
    "#,
    )
    .expect("Interpret failed");

    let mut handle: Option<WrenCallHandle> = None;
    vm.context(|ctx| {
        let call_ref = ctx.make_call_ref("test_handle", "Test", "calc(_)").unwrap();
        handle = call_ref.leak();
    });

    assert!(handle.is_some());

    vm.context(|ctx| {
        let result = handle.unwrap().call::<_, f64>(ctx, 4.0).unwrap();
        assert_eq!(result, 16.0);
    });
}

// Handle can be sent to another thread. Required if we are to process fibers in a thread pool.
#[test]
fn test_handle_thread_send() {
    let mut vm = WrenBuilder::new().build();
    vm.interpret(
        "test_handle",
        r#"
    var a = 42
    "#,
    )
    .expect("Interpret failed");

    vm.context(|ctx| {
        let a = ctx.get_var("test_handle", "a").map(|r| r.leak()).unwrap();

        let join = thread::spawn(move || {
            assert!(a.is_some());
        });

        join.join().unwrap();
    });
}

// We should be able to retrieve a variable via a property, and use that as the receiver of a call reference.
#[test]
fn test_property_as_receiver() {
    let mut vm = WrenBuilder::new().build();
    vm.interpret(
        "test_handle",
        r#"
    class Inner {
      construct new() {}
      callme() { 7 }
    }

    class Outer {
      static inner { Inner.new() }
    }
    "#,
    )
    .expect("Interpret failed");

    vm.context(|ctx| {
        let prop_call = ctx.make_call_ref("test_handle", "Outer", "inner").unwrap();
        let receiver = prop_call.call::<_, WrenRef>(ctx, ()).unwrap();

        let callme_sym = FnSymbolRef::compile(ctx, "callme()").unwrap();
        let callme = WrenCallRef::new(receiver, callme_sym);

        assert_eq!(callme.call::<_, i32>(ctx, ()), Some(7));
        assert_eq!(callme.call::<_, i32>(ctx, ()), Some(7));

        let callme = callme.leak().unwrap();
        assert_eq!(callme.call::<_, i32>(ctx, ()), Some(7));
        assert_eq!(callme.call::<_, i32>(ctx, ()), Some(7));
    });
}
