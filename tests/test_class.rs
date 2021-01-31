use rust_wren::prelude::*;

#[wren_class]
#[derive(Debug, Clone, Copy)]
struct Vector2 {
    #[getset]
    x: f64,
    #[getset]
    y: f64,
}

#[wren_methods]
impl Vector2 {
    #[construct]
    fn new(x: f64, y: f64) -> Self {
        Vector2 { x, y }
    }

    fn x(&self) -> f64 {
        self.x
    }

    fn y(&self) -> f64 {
        self.y
    }

    fn zero() -> Self {
        Vector2::new(0.0, 0.0)
    }

    fn test() -> Self {
        Vector2::new(7.0, 11.0)
    }

    #[method(name = fooBar)]
    fn foo_bar() {
        println!("foo_bar()");
    }

    #[method(name = fooBar)]
    fn foo_bar_1(a: f64) {
        println!("foo_bar_1({})", a);
    }

    #[method(name = fooBar)]
    fn foo_bar_2(a: f64, b: f64) {
        println!("foo_bar_2({}, {})", a, b);
    }

    fn magnitude(&self) -> f64 {
        (self.x.powf(2.0) + self.y.powf(2.0)).sqrt()
    }

    fn dot(&self, rhs: &WrenCell<Vector2>) -> f64 {
        let other = rhs.borrow();
        self.x * other.x + self.y * other.y
    }
}

const VECTOR: &str = r#"
foreign class Vector2 {
    construct new(x, y) {}
    foreign x
    foreign x=(value)
    foreign y
    foreign y=(value)
    foreign x()
    foreign y()
    foreign static zero()
    foreign static test()
    foreign static fooBar()
    foreign static fooBar(a)
    foreign static fooBar(a, b)
    foreign magnitude()
    foreign dot(rhs)
}
"#;

#[test]
fn test_wren_class() {
    let mut vm = WrenBuilder::new()
        .with_module("test", |m| {
            m.register::<Vector2>();
        })
        .build();
    vm.interpret("test", VECTOR).expect("Interpret error");

    println!("Rust: {}", Vector2::new(7.0, 11.0).magnitude());

    vm.interpret(
        "test",
        r#"
            var a = Vector2.new(7.0, 11.0)
            System.print("Wren: %(a.magnitude())")

            var dot = Vector2.new(-6, 8).dot(Vector2.new(5, 12))
            System.print("dot product: %(dot)")
            if (dot != 66) {
                Fiber.abort("Incorrect dot product")
            }

           var zero = Vector2.zero()
           if (zero.x() != 0 || zero.y() != 0) {
               Fiber.abort("Unexpected zero() Vector2(%(zero.x()), %(zero.y()))")
           }
           
           var someValue = Vector2.test()
           if (someValue.x() != 7 || someValue.y() != 11) {
               Fiber.abort("Unexpected test() Vector2(%(zero.x()), %(zero.y()))")
           }
           Vector2.zero()
           Vector2.zero()
           Vector2.zero()

           // Function with different names in Wren and Rust.
           Vector2.fooBar()
           Vector2.fooBar(1)
           Vector2.fooBar(1, 2)
    "#,
    )
    .expect("Interpret error");

    drop(vm);
    println!("After vm drop");
}

#[test]
fn test_property() {
    let mut vm = WrenBuilder::new()
        .with_module("test", |m| {
            m.register::<Vector2>();
        })
        .build();

    vm.interpret("test", VECTOR).expect("Interpret error");
    vm.interpret("test", include_str!("test.wren"))
        .expect("Load test utils failed");

    vm.interpret(
        "test",
        r#"
    var a = Vector2.new(3, 9)

    Test.assertEq(a.x, 3, "Vector2.x")
    Test.assertEq(a.y, 9, "Vector2.y")

    // Property assignment return must be the assigned value, by convention.
    Test.assertEq(a.x = 7, 7, "Vector2.x")
    Test.assertEq(a.y = 11, 11, "Vector2.y")
    Test.assertEq(a.x, 7, "Vector2.x")
    Test.assertEq(a.y, 11, "Vector2.y")
    System.print("Vector2(%(a.x), %(a.y))")
    "#,
    )
    .expect("Interpret error");
}
