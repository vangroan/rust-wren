use rust_wren::{handle::WrenHandle, prelude::*, types::WrenType, WrenContext, WrenError};

#[wren_class]
struct Foo;

#[wren_methods]
impl Foo {
    #[construct]
    fn new() -> Self {
        Self
    }

    fn convert(#[ctx] ctx: &mut WrenContext, left: f64, buf: WrenList, right: f64) {
        println!("left: {}", left);
        println!("right: {}", right);
        println!("{:?}", buf);

        let converted = buf.to_vec::<f64>(ctx).expect("failed to convert Wren list to Vec<f64>");

        assert_eq!(left, 7.0);
        assert_eq!(right, 11.0);
        assert_eq!(converted, vec![1., 2., 3., 4., 5., 6., 7., 8., 9., 10.]);
    }

    fn makelist(#[ctx] ctx: &mut WrenContext) -> rust_wren::Result<WrenList> {
        let data = vec![1., 2., 3., 4., 5., 6., 7., 8., 9., 10.];
        WrenList::from_vec::<f64>(ctx, data).map_err(|err| foreign_error!(err))
    }

    fn acceptlist(list: Option<WrenList>) {
        assert!(list.is_some(), "Wren list is None");
        println!("{:?}", list);
    }
}

#[test]
fn test_list_new() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret("test", include_str!("test.wren")).unwrap();
    vm.interpret(
        "test_list",
        r#"
    import "test" for Test

    class TestList {
      static sumList(list) {
        Test.assertEq(list[0], 7, "List element does not match")
        Test.assertEq(list[1], 11, "List element does not match")
        Test.assertEq(list[2], 13, "List element does not match")
      }
    } 
    "#,
    )
    .unwrap();

    vm.context_result(|ctx| {
        let mut list = WrenList::new(ctx);
        assert!(list.is_empty(ctx));
        list.push(ctx, 7_f64);
        list.push(ctx, 11_f64);
        list.push(ctx, 13_f64);
        assert_eq!(list.len(ctx), 3);
        assert!(!list.is_empty(ctx));

        let call_ref = ctx.make_call_ref("test_list", "TestList", "sumList(_)")?;
        call_ref.call::<_, ()>(ctx, list)?;

        Ok(())
    })
    .unwrap();
}

#[test]
fn test_vec_from_wren() {
    let mut vm = WrenBuilder::new()
        .with_module("test_list", |m| {
            m.register::<Foo>();
        })
        .build();

    vm.interpret("test", include_str!("test.wren")).unwrap();
    vm.interpret(
        "test_list",
        r#"
        foreign class Foo {
            construct new() {}
            foreign static convert(left, buf, right)
        }
        "#,
    )
    .expect("Interpret error");

    vm.interpret("test_list", r#"Foo.convert(7, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 11)"#)
        .expect("Call convert failed");
}

#[test]
fn test_vec_to_wren() {
    let mut vm = WrenBuilder::new()
        .with_module("test_list", |m| {
            m.register::<Foo>();
        })
        .build();

    vm.interpret("test", include_str!("test.wren")).unwrap();
    vm.interpret(
        "test_list",
        r#"
        foreign class Foo {
            construct new() {}
            foreign static makelist()
        }
        "#,
    )
    .expect("Interpret error");

    vm.interpret(
        "test_list",
        r#"
        import "test" for Test

        var x = Foo.makelist()
        for (i in 1..10) {
            Test.assertEq(x[i-1], i, "List element does not match")
        }
    "#,
    )
    .expect("Call makelist failed");
}

#[test]
fn test_list_handle() {
    let mut vm = WrenBuilder::new()
        .with_module("test_list", |m| {
            m.register::<Foo>();
        })
        .build();

    vm.interpret("test", include_str!("test.wren")).unwrap();
    vm.interpret(
        "test_list",
        r#"
        foreign class Foo {
            construct new() {}
            foreign static acceptlist(x)
        }
        "#,
    )
    .expect("Interpret error");

    vm.interpret(
        "test_list",
        r#"
        var x = [1, 2, 3, 4, 5, 6, 7, 8, 9]
        Foo.acceptlist(x)
        "#,
    )
    .expect("Interpret error");
}

/// Push element to back of Wren list from Rust
#[test]
fn test_list_handle_push() {
    let mut vm = WrenBuilder::new().build();

    vm.interpret("test", include_str!("test.wren")).unwrap();

    vm.interpret(
        "test_list",
        r#"
        var x = []
        "#,
    )
    .expect("Interpret error");

    vm.context_result(|ctx| {
        let wren_ref = ctx.get_var("test_list", "x")?;
        let wren_handle: WrenHandle = wren_ref.leak()?;
        let mut wren_list: WrenList = unsafe { WrenList::from_handle_unchecked(wren_handle) };

        wren_list.push(ctx, 1_f64);
        wren_list.push(ctx, 2_f64);
        wren_list.push(ctx, 3_f64);

        wren_list.set(ctx, 0, 99);

        assert_eq!(wren_list.len(ctx), 3);

        Ok(())
    })
    .unwrap();

    vm.interpret(
        "test_list",
        r#"
        import "test" for Test

        Test.assertEq(x[0], 99, "List element does not match")
        Test.assertEq(x[1], 2, "List element does not match")
        Test.assertEq(x[2], 3, "List element does not match")

        Test.assertEq(x.count, 3, "List count is incorrect")

        x.add("added element")
        "#,
    )
    .expect("Interpret error");

    vm.context_result(|ctx| {
        let wren_ref = ctx.get_var("test_list", "x")?;
        let wren_handle: WrenHandle = wren_ref.leak()?;
        let wren_list: WrenList = unsafe { WrenList::from_handle_unchecked(wren_handle) };

        assert_eq!(wren_list.get::<i32>(ctx, 0)?, Some(99));
        assert_eq!(wren_list.get::<i32>(ctx, 1)?, Some(2));
        assert_eq!(wren_list.get::<i32>(ctx, 2)?, Some(3));
        assert_eq!(wren_list.get::<String>(ctx, 3)?, Some("added element".to_owned()));

        Ok(())
    })
    .unwrap();
}

#[test]
fn test_list_to_vec() {
    let mut vm = WrenBuilder::new().build();
    vm.interpret("test", include_str!("test.wren")).unwrap();

    vm.interpret(
        "test_list",
        r#"
        var x = ["spruce", "maple", "willow"]
        "#,
    )
    .expect("Interpret error");

    vm.context_result(|ctx| {
        let wren_list = ctx.get_list("test_list", "x")?;
        let trees = wren_list.to_vec::<String>(ctx)?;
        assert_eq!(&trees, &["spruce", "maple", "willow"]);

        Ok(())
    })
    .expect("Context error");
}

#[test]
fn test_list_clone_to() {
    let mut vm = WrenBuilder::new().build();
    vm.interpret("test", include_str!("test.wren")).unwrap();

    vm.interpret(
        "test_list",
        r#"
        var x = ["spruce", "maple", "willow"]
        "#,
    )
    .expect("Interpret error");

    vm.context_result(|ctx| {
        let wren_list = ctx.get_list("test_list", "x")?;
        let mut buf: Vec<String> = vec!["".to_string(); 3];
        let size = wren_list.clone_to::<String>(ctx, &mut buf)?;
        assert_eq!(&buf, &["spruce", "maple", "willow"]);
        assert_eq!(size, 3);

        // Type Error
        let mut buf: Vec<i32> = vec![0; 3];
        let result = wren_list.clone_to::<i32>(ctx, &mut buf);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(WrenError::SlotType {
                expected: WrenType::Number,
                actual: WrenType::String
            })
        ));

        Ok(())
    })
    .expect("Context error");
}
