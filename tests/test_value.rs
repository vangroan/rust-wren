use rust_wren::prelude::*;

#[wren_class]
#[derive(Debug)]
struct Foo(f64);

#[wren_methods]
impl Foo {
    #[construct]
    fn new(value: f64) -> Self {
        Foo(value)
    }

    fn other(&self, rhs: &WrenCell<Foo>) {
        {
            let rhs_foo = rhs.borrow();
            println!("{:?}", rhs_foo);
        }

        {
            let mut rhs_foo_mut = rhs.borrow_mut();
            rhs_foo_mut.0 = self.0
        }
    }

    fn other_mut(&self, rhs: &mut WrenCell<Foo>) {
        {
            let rhs_foo = rhs.borrow();
            println!("{:?}", rhs_foo);
        }

        {
            let mut rhs_foo_mut = rhs.borrow_mut();
            rhs_foo_mut.0 = self.0
        }
    }

    fn string(&self, s: String) -> String {
        println!("Rust: Foo.string(\"{}\")", s);
        format!("{} {}", s, self.0)
    }

    fn str(&self, s: &str) -> String {
        println!("Rust: Foo.str(\"{}\")", s);
        format!("{} {}", s, self.0)
    }

    fn optional(&self, val: Option<String>) -> Option<String> {
        println!("Rust: Foo.optional({:?})", val);
        match val {
            Some(_) => None,
            None => Some("Not Null".to_owned()),
        }
    }

    fn multi_borrow(&self, foo: &WrenCell<Foo>) {
        // Should fail when both self and foo are the same foreign value
        let _eat_me = foo.borrow_mut();
    }
}

const FOO: &str = r#"
foreign class Foo {
    construct new(value) {}

    foreign other(rhs)
    foreign other_mut(rhs)
    foreign string(s)
    foreign str(s)
    foreign optional(val)
    foreign multi_borrow(foo)
}
"#;

#[test]
fn test_wren_cell_from_wren() {
    let mut vm = WrenBuilder::new()
        .with_module("test_value", |m| {
            m.register::<Foo>();
        })
        .build();

    vm.interpret("test_value", FOO).expect("Interpret lines");
    vm.interpret(
        "test_value",
        r#"
    var lhs = Foo.new(1)
    var rhs = Foo.new(2)
    lhs.other(rhs)
    lhs.other_mut(rhs)
    "#,
    )
    .expect("Interpret failed");
}

/// Type check for incorrect foreign type.
#[test]
#[should_panic]
fn test_wren_cell_incorrect_type() {
    #[wren_class]
    struct Bar;

    #[wren_methods]
    impl Bar {
        #[construct]
        fn new() -> Self {
            Self
        }
    }

    let mut vm = WrenBuilder::new()
        .with_module("test_value", |m| {
            m.register::<Foo>();
            m.register::<Bar>();
        })
        .build();

    vm.interpret("test_value", FOO).expect("Interpret failed");
    vm.interpret(
        "test_value",
        r#"
    foreign class Bar {
        construct new() {}
    }
    "#,
    )
    .expect("Interpret failed");

    vm.interpret(
        "test_value",
        r#"
    var lhs = Foo.new(1)
    var rhs = Bar.new()
    lhs.other(rhs)
    lhs.other_mut(rhs)
    "#,
    )
    .expect("Interpret failed");
}

#[test]
fn test_string() {
    let mut vm = WrenBuilder::new()
        .with_module("test_value", |m| {
            m.register::<Foo>();
        })
        .build();

    vm.interpret("test_value", FOO).expect("Interpret lines");
    vm.interpret(
        "test_value",
        r#"
    var s1 = Foo.new(2).string("Test copied String")
    if (s1 != "Test copied String 2") {
        Fiber.abort("Unexpected result string \"%(s1)\"")
    }

    var s2 = Foo.new(3).str("Test borrowed &str")
    if (s2 != "Test borrowed &str 3") {
        Fiber.abort("Unexpected result string \"%(s2)\"")
    }    
    "#,
    )
    .expect("Interpret failed");
}

#[test]
fn test_unicode() {
    let mut vm = WrenBuilder::new()
        .with_module("test_value", |m| {
            m.register::<Foo>();
        })
        .build();

    vm.interpret("test_value", FOO).expect("Interpret lines");
    vm.interpret(
        "test_value",
        r#"
    var s1 = Foo.new(2).string("Test copied String Ⅰ Ⅱ Ⅲ ⏰")
    if (s1 != "Test copied String Ⅰ Ⅱ Ⅲ ⏰ 2") {
        Fiber.abort("Unexpected result string \"%(s1)\"")
    }

    var s2 = Foo.new(3).str("Test borrowed &str Ⅰ Ⅱ Ⅲ ⏰")
    if (s2 != "Test borrowed &str Ⅰ Ⅱ Ⅲ ⏰ 3") {
        Fiber.abort("Unexpected result string \"%(s2)\"")
    }    
    "#,
    )
    .expect("Interpret failed");
}

#[test]
fn test_nullable() {
    let mut vm = WrenBuilder::new()
        .with_module("test_value", |m| {
            m.register::<Foo>();
        })
        .build();

    vm.interpret("test_value", FOO).expect("Interpret lines");
    vm.interpret(
        "test_value",
        r#"
    var s1 = Foo.new(2).optional("Argument not null")
    if (s1 != null) {
        Fiber.abort("Unexpected result \"%(s1)\"")
    }
  
    var s2 = Foo.new(3).optional(null)
    if (s2 != "Not Null") {
        Fiber.abort("Unexpected result \"%(s2)\"")
    }
    "#,
    )
    .expect("Interpret failed");
}

#[test]
#[should_panic]
fn test_multiple_borrow() {
    let mut vm = WrenBuilder::new()
        .with_module("test_value", |m| {
            m.register::<Foo>();
        })
        .build();

    vm.interpret("test_value", FOO).expect("Interpret lines");
    vm.interpret(
        "test_value",
        r#"
    var s1 = Foo.new(1)
    s1.multi_borrow(s1)
    "#,
    )
    .expect("Interpret failed");
}
