use rust_wren::prelude::*;

#[wren_class]
#[derive(Debug, Clone, Copy)]
struct Vector2 {
    x: f64,
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

    fn magnitude(&self) -> f64 {
        (self.x.powf(2.0) + self.y.powf(2.0)).sqrt()
    }

    fn dot(&self, rhs: &WrenCell<Vector2>) -> f64 {
        let other = rhs.borrow();
        self.x * other.x + self.y * other.y
    }
}

#[test]
fn test_wren_class() {
    let mut vm = WrenBuilder::new()
        .with_module("test", |m| {
            m.register::<Vector2>();
        })
        .build();

    println!("Rust: {}", Vector2::new(7.0, 11.0).magnitude());

    vm.interpret(
        "test",
        r#"
            foreign class Vector2 {
                construct new(x, y) {}
                foreign x()
                foreign y()
                foreign static zero()
                foreign static test()
                foreign magnitude()
                foreign dot(rhs)
            }

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
    "#,
    )
    .expect("Interpret error");

    drop(vm);
    println!("After vm drop");
}
