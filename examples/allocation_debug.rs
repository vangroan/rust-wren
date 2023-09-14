//! Demonstration of detecting allocation bugs.
use log::info;
use rust_wren::module::FileModuleLoader;
use rust_wren::prelude::*;
use rust_wren::troubleshoot::{assert_all_deallocated, dump_allocations};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    info!("Starting...");

    // Example has to be run from project root directory.
    let loader = FileModuleLoader::with_root(::std::env::current_dir().unwrap().join("examples"));

    let mut vm = WrenBuilder::new().with_module_loader(loader).build();

    vm.context_result(|ctx| {
        let mut list = WrenList::new(ctx);
        list.push(ctx, 1);
        list.push(ctx, 2);
        list.push(ctx, 3);

        // Print current allocations to logs.
        dump_allocations();

        Ok(())
    })
    .expect("context failed");

    // Catch allocation issues when copying static string.
    vm.interpret(
        "my_module",
        r#"
    class Foobar {}
    "#,
    )
    .expect("interpret failed");

    vm.interpret(
        "main",
        r#"
    import "allocation_debug"
    import "my_module" for Foobar
    var x = [1, 2, 3]
    "#,
    )
    .expect("interpret failed");

    // Bug where clearing an empty list leaks memory.
    vm.interpret(
        "main",
        r#"
    var y = []
    for (i in 0...10) {
      y.clear()
    }
    "#,
    )
    .expect("interpret failed");

    info!("Done...");
    drop(vm);

    assert_all_deallocated();
}
