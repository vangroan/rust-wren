#[macro_use]
extern crate rust_wren_derive;

use rust_wren::{prelude::*, HelloMacro};

const SOURCE: &str = r##"
System.print("I am running in a VM!")

class Foo {
    static bar() {
        System.print("foobar")
    }
}

Foo.bar()
"##;

const MODULE: &str = r##"
class Engine {
    construct new() {}
    foreign log(message)
}

foreign class Vector3 {
    construct new() {}
    foreign contents()
}
"##;

#[wren_class(name=Foo)]
#[derive(HelloMacro, Debug)]
struct ProcFoo {}

#[wren_methods]
impl ProcFoo {
    #[construct]
    fn new() -> Self {
        ProcFoo {}
    }
}

fn main() {
    let mut vm = WrenBuilder::new()
        .with_module("engine", |module| {
            module.register::<ProcFoo>();
        })
        .build();

    vm.interpret("my_module", SOURCE).expect("Interpret error");
    println!("slot count: {}", vm.slot_count());

    vm.interpret("engine", MODULE).unwrap();
    vm.interpret(
        "engine",
        r##"
        Engine.new().log("foobar")
        Vector3.new().contents()
        "##,
    )
    .unwrap();

    ProcFoo::hello_macro();
    println!("{:?} has wren name {}", ProcFoo {}, ProcFoo::NAME);
}
