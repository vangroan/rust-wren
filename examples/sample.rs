use rust_wren::WrenBuilder;

const SOURCE: &str = "
System.print(\"I am running in a VM!\")

class Foo {
  static bar() {
    Fiber.abort(\"eat me\")
  }
}

Foo.bar()
";

fn main() {
    let vm = WrenBuilder::new().build();

    vm.interpret("my_module", SOURCE).expect("Interpret error");
}
