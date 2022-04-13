//! Passing a list from within Wren to Rust.
use rust_wren::prelude::*;
use rust_wren::WrenContext;

#[derive(Debug)]
#[wren_class]
struct TreeNamer {}

#[wren_methods]
impl TreeNamer {
    #[construct]
    fn new() -> Self {
        TreeNamer {}
    }

    /// Add tree names from static method.
    #[method(name = addStatic)]
    fn add_static(#[ctx] ctx: &mut WrenContext, mut list: WrenList) {
        list.push(ctx, "spruce");
        list.push(ctx, "maple");
        list.push(ctx, "birch");
    }

    /// Add tree names from instance method.
    #[method(name = addInstance)]
    fn add_instance(&self, #[ctx] ctx: &mut WrenContext, mut list: WrenList) {
        list.push(ctx, "acacia");
        list.push(ctx, "baobab");
        list.push(ctx, "marula");
    }

    #[method(name = rustPrint)]
    fn rust_print(#[ctx] ctx: &mut WrenContext, list: WrenList) -> rust_wren::Result<()> {
        let trees = list.to_vec::<String>(ctx).map_err(|err| foreign_error!(err))?;
        println!("Rust: {:?}", trees);
        Ok(())
    }
}

const DECLARE_SCRIPT: &str = r#"
foreign class TreeNamer {
  construct new() {}
  foreign static addStatic(ls)
  foreign addInstance(ls)
  foreign static rustPrint(ls)
}
"#;

fn main() {
    let mut vm: WrenVm = WrenBuilder::new()
        .with_module("example_list", |m| {
            m.register::<TreeNamer>();
        })
        .build();

    vm.interpret("example_list", DECLARE_SCRIPT).expect("Interpret failed");

    vm.interpret(
        "example_list",
        r#"
    var x = []
    
    TreeNamer.addStatic(x)
    var namer = TreeNamer.new()
    namer.addInstance(x)
    System.print("Wren: %(x)")

    TreeNamer.rustPrint(x)
    "#,
    )
    .expect("Interpret failed");
}
