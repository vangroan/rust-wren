//! The mandelbrot example from the Wren repository.
use rust_wren::prelude::*;

const SCRIPT: &str = include_str!("../wren/example/mandelbrot.wren");

fn main() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret("mandelbrot", SCRIPT).expect("Interpret error");
}
