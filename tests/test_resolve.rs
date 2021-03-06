use rust_wren::{prelude::*, module::{UnitModuleResolver, FileModuleLoader}};

#[test]
fn test_module_resolve() {
    let mut vm = WrenBuilder::new()
        .with_module_resolver(UnitModuleResolver::default())
        .build();

    vm.interpret(
        "module_1",
        r#"
    class Foo {}
    "#,
    )
    .expect("Interpret failed");

    vm.interpret(
        "module_2",
        r#"
    import "module_1" for Foo
    "#,
    )
    .expect("Interpret failed");
}

#[test]
fn test_module_load() {
    let mut vm = WrenBuilder::new()
        .with_module_resolver(UnitModuleResolver::default())
        .with_module_loader(FileModuleLoader::with_root(
            std::env::current_dir().unwrap().join("tests"),
        ))
        .build();

    vm.interpret(
        "module_2",
        r#"
    import "module_1" for Foo
    System.print("%(Foo)")
    "#,
    )
    .expect("Interpret failed");
}
