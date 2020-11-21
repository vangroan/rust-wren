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

    // fn zero() -> Self {
    //     Vector2::new(0.0, 0.0)
    // }

    fn magnitude(&self) -> f64 {
        (self.x.powf(2.0) + self.y.powf(2.0)).sqrt()
    }
}

#[test]
fn test_wren_class() {
    let vm = WrenBuilder::new()
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
                foreign magnitude()
            }

            var a = Vector2.new(7.0, 11.0)
            System.print("Wren: %(a.magnitude())")
    "#,
    )
    .expect("Interpret error");

    drop(vm);
    println!("After vm drop");
}

// =========================
