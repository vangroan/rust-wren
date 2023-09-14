
# `rust-wren` examples

## Feature Samples

- [basic](basic.rs) Basic usage example of setting up a default VM, interpreting a script and calling a Wren function from Rust code.
- [iterator](iterator.rs) Implementing the Wren iterator protocol from a Rust foreign class.
- [list](list.rs) Usage of lists. Creating a list in Wren, passing it to Rust. Creating a list from Rust, passing it to a Wren method.
- [mandelbrot](mandelbrot.rs) The mandelbrot example from the Wren repository.

## Known Issues

- [issue_construct](issue_construct.rs) Demonstration of the issue where a foreign class' constructor
  is not called when creating the instance from Rust.
