use rust_wren::prelude::*;

#[wren_class]
#[derive(Debug)]
struct NotZero(f64);

#[wren_methods]
impl NotZero {
    #[construct]
    fn new(value: f64) -> Self {
        NotZero(value)
    }
}

impl Drop for NotZero {
    fn drop(&mut self) {
        println!("Dropping {:?}", self);
        if self.0.abs() <= ::std::f64::EPSILON {
            panic!("NotZero is zero!");
        }
    }
}

/// Test that the generated allocation and finalisation methods
/// only drop values that have been initialised.
///
/// Fails if an uninitialised value is dropped.
#[test]
fn test_memory_safety() {
    let vm = WrenBuilder::new()
        .with_module("test_memory_safety", |m| {
            m.register::<NotZero>();
        })
        .build();

    vm.interpret(
        "test_memory_safety",
        r#"
    foreign class NotZero {
        construct new(val) {}

        static test() {
            var inStatic = NotZero.new(3)
        }
    }

    NotZero.new(1)
    var notZero = NotZero.new(2)
    NotZero.test()
    "#,
    )
    .expect("Interpret error");

    drop(vm);
}
