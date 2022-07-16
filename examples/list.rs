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

    // Example 1
    //
    // Create a list in Wren, pass a reference to Rust,
    // add strings from Rust into the Wren list.
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

    // Example 2
    //
    // Create a Wren list in Rust, add strings,
    // pass it to Wren to add more strings.
    vm.context_result(|ctx| {
        let mut list = WrenList::new(ctx);
        list.push(ctx, "mayten");
        list.push(ctx, "avocado");
        list.push(ctx, "jacaranda");

        let add_static = ctx.make_call_ref("example_list", "TreeNamer", "addStatic(_)")?;
        add_static.call::<_, ()>(ctx, &list)?;

        // Rust Print
        println!("Rust: {:?}", list.to_vec::<String>(ctx)?);

        // Wren Print
        let print_call = ctx.make_call_ref("example_list", "System", "print(_)")?;
        print_call.call::<_, ()>(ctx, &list)?;

        Ok(())
    })
    .expect("Context block failed");
}
