use rust_wren::{
    handle::{FnSymbol, WrenCallRef},
    prelude::*,
};

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
        let print_fn = FnSymbol::compile(ctx, "print()");
        let call_handle = WrenCallRef::new(test_class, print_fn);

        println!("Rust: Calling TestHandle.print()");
        call_handle.call::<_, ()>(ctx, ());
    });

    vm.context(|ctx| {
        // Static call looks up class declaration as variable.
        let test_class = ctx.get_var("test_handle", "TestHandle").unwrap();
        let print_fn = FnSymbol::compile(ctx, "withArgs(_,_,_)");
        let call_handle = WrenCallRef::new(test_class, print_fn);

        println!("Rust: Calling TestHandle.withArgs(_,_,_)");
        assert_eq!(
            call_handle.call::<_, f64>(ctx, (3.0, 7.0, 11.0)),
            Some(21.0)
        );
    });

    drop(vm);
}

/// Should call method on foreign class.
#[test]
fn test_foreign_call() {
    let mut vm = WrenBuilder::new()
        .with_module("test_handle", |module| module.register::<MoveMe>())
        .build();

    vm.interpret("test_handle", MOVE_ME).unwrap();
    vm.interpret("test_handle", r#"var m = MoveMe.new(7)"#)
        .unwrap();

    vm.context(|ctx| {
        // Instance method
        let move_me_obj = ctx.get_var("test_handle", "m").unwrap();
        let func = FnSymbol::compile(ctx, "inner()");
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
    vm.interpret("test_handle", r#"var m = MoveMe.new(11)"#)
        .unwrap();

    vm.context(|ctx| {
        // Instance method
        let move_me_obj = ctx.get_var("test_handle", "m").unwrap();
        let func = FnSymbol::compile(ctx, "one(_)");
        let call_handle = WrenCallRef::new(move_me_obj, func);

        println!("Rust: Calling MoveMe.one(_)");
        let result: f64 = call_handle.call::<f64, f64>(ctx, 7.0).unwrap();
        assert_eq!(result, 18.0);
    });

    vm.context(|ctx| {
        // Instance method
        let move_me_obj = ctx.get_var("test_handle", "m").unwrap();
        let func = FnSymbol::compile(ctx, "two(_,_)");
        let call_handle = WrenCallRef::new(move_me_obj, func);

        println!("Rust: Calling MoveMe.two(_,_)");
        let result: f64 = call_handle.call::<_, f64>(ctx, (7.0, 3.0)).unwrap();
        assert_eq!(result, 21.0);
    });
}

#[test]
fn test_non_existing() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret("test_handle", "").unwrap();

    vm.context(|ctx| {
        assert!(ctx.get_var("test_handle", "m").is_none());
        assert!(ctx.get_var("unknown", "m").is_none());
    });
}
