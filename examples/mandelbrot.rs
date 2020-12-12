//! The mandelbrot example from the Wren repository.
use rust_wren::WrenBuilder;

fn main() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret(
        "syntax_example",
        include_str!("../wren/example/mandelbrot.wren"),
    )
    .expect("Interpret error");
}
