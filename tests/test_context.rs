use rust_wren::prelude::*;

/// Should check whether a variable exists or not.
#[test]
fn test_has_variable() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret(
        "test_context",
        r#"
    var One = "one"

    class Two {}
    "#,
    )
    .expect("Interpret failed");

    vm.context(|ctx| {
        assert!(ctx.has_var("test_context", "One"));
        assert!(ctx.has_var("test_context", "Two"));
        assert!(!ctx.has_var("test_context", "Three"));
        assert!(!ctx.has_var("unknown", "One"));
    });
}

#[test]
fn test_has_module() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret("test_context", "").expect("Interpret failed");

    vm.context(|ctx| {
        assert!(ctx.has_module("test_context"));
        assert!(!ctx.has_module("unknown"));
    });
}
