//! Demonstration of the issue where a foreign class' constructor is not called when
//! creating the instance from Rust.
use rust_wren::prelude::*;

#[wren_class]
struct GameObject {
    #[getset]
    id: i32,
}

#[wren_methods]
impl GameObject {
    #[construct]
    fn new(id: i32) -> Self {
        println!("Rust factory function: {}", id);
        GameObject { id }
    }

    #[method(name = createInstance)]
    fn create_instance(id: i32) -> GameObject {
        // The `construct` method declared in Wren will not be called.
        GameObject::new(id)
    }
}

const DECLARE_SCRIPT: &str = r#"
foreign class GameObject {
  construct new(id) {
    System.print("Wren constructor: %(id)")
  }
  foreign static createInstance(id)
}
"#;

fn main() {
    let mut vm = WrenBuilder::new()
        .with_module("main", |m| m.register::<GameObject>())
        .build();

    vm.interpret("main", DECLARE_SCRIPT).unwrap();

    vm.interpret(
        "example",
        r#"
    import "main" for GameObject

    var obj1 = GameObject.new(1)
    //> Rust factory function: 1
    //> Wren constructor: 1

    var obj2 = GameObject.createInstance(2)
    //> Rust factory function: 2
    "#,
    )
    .unwrap();
}
