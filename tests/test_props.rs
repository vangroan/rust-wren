//! Test property generation.
use rust_wren::prelude::*;

#[wren_class]
#[derive(Debug)]
struct Foo {
    #[get]
    bar: String,
    #[set]
    baz: String,
    #[getset]
    bar_baz: String,
}

#[wren_methods]
impl Foo {
    #[construct]
    fn new(bar: &str) -> Self {
        Self {
            bar: bar.to_owned(),
            baz: String::new(),
            bar_baz: "DEFAULT BAR_BAZ".to_owned(),
        }
    }

    #[method(name = getBaz)]
    fn get_baz(&self) -> String {
        self.baz.clone()
    }
}

const FOO: &str = r#"
foreign class Foo {
    foreign bar
    foreign baz=(value)
    foreign bar_baz
    foreign bar_baz=(value)

    construct new(bar) {}
    foreign getBaz()
}
"#;

#[test]
fn test_properties() {
    let mut vm = WrenBuilder::new()
        .with_module("test_properties", |module| {
            module.register::<Foo>();
        })
        .build();

    vm.interpret("test_properties", FOO).expect("Interpret failed");
    vm.interpret("test", include_str!("test.wren"))
        .expect("Interpret failed");

    vm.interpret(
        "test_properties",
        r#"
    import "test" for Test

    var a = Foo.new("BAR")

    Test.assertEq(a.bar, "BAR", "Foo.bar")
    Test.shouldFailWith("a.bar assignment", "Foo does not implement 'bar=(_)'.") {
        a.bar = "INVALID"
    }

    // Property assignment return must be the assigned value, by convention.
    Test.assertEq(a.baz = "BAZ", "BAZ", "Foo.baz=")
    Test.shouldFailWith("a.baz get", "Foo does not implement 'baz'.") {
        var baz = a.baz
    }

    Test.assertEq(a.bar_baz, "DEFAULT BAR_BAZ", "Foo.bar_baz")
    // Property assignment return must be the assigned value, by convention.
    Test.assertEq(a.bar_baz = "BAR_BAZ", "BAR_BAZ", "Foo.bar_baz=")
    Test.assertEq(a.bar_baz, "BAR_BAZ", "Foo.bar_baz")

    // Ensure we haven't mutated the others fields.
    Test.assertEq(a.bar, "BAR", "Foo.bar")
    Test.assertEq(a.getBaz(), "BAZ", "Foo.getBaz()")
    "#,
    )
    .expect("Interpret failed");
}
