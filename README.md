
# 🦀 Rust ↔ Wren 🐦

Wrapper library to embed the [Wren](https://wren.io) scripting language inside a Rust application.

⚠ Warning: This library is under heavy development. Many features are missing or broken. Every minor version bump will break the API.

## Build

### Windows

Building on Windows requires Visual Studio 2019 to build the Wren C library.

```
msbuild projects\vs2019\wren.vcxproj /property:Configuration="Debug 64bit" /property:Platform=x64
```

### WSL Ubuntu

Install clang for bindgen.

```sh
sudo apt update
sudo apt install clang
```

## Usage

For examples on usage, see the [examples](./examples/) folder.

Basic usage to interpret a script.

```rust
use rust_wren::prelude::*;

fn main() {
    // Build a Wren VM instance
    let mut vm = WrenBuilder::new().build();

    // Script is a &str that will be copied into Wren's compiler.
    let script = r#"
    System.print("Hello, World")
    "#;
    
    // Script must be executed as a module.
    vm.interpret("my_module", script).expect("Interpret error");
}
```

Call a Wren function from Rust code.

```rust
use rust_wren::prelude::*;

fn main() {
    // Build a Wren VM instance
    let mut vm = WrenBuilder::new().build();

    // Execute script to declare the class in the module.
    vm.interpret("main", r#"
    class Game {
      construct new() {
        _me = "Wren 🐦"
      }
      
      greet(name) {
        System.print("Hello, %(name)! From %(_me)")
      }
    }
    
    var game = Game.new()
    "#).expect("Interpret error");
    
    // Make a call handle to call it from Rust.
    vm.context_result(|ctx| {
        let greet_func = ctx.make_call_ref("main", "game", "greet(_)")?;
        
        greet_func.call::<_, ()>(ctx, "Rust 🦀")?;
        
        Ok(())
    }).expect("Context error");
}
```

Declare a custom Wren class in Rust.

```rust
use rust_wren::prelude::*;

#[wren_class(name=Engine)]
struct WrenEngine {
    #[get]
    version: u32,
}

#[wren_methods]
impl WrenEngine {
    #[construct]
    fn new() -> Self {
        WrenEngine { version: 1 }
    }

    fn add(&self, lhs: u32, rhs: u32) -> u32 {
        lhs + rhs
    }
}

const DECLARE_ENGINE: &str = r#"
foreign class Engine {
  construct new() {}
  foreign version
  foreign add(lhs, rhs)
}
"#;

fn main() {
    let mut vm = WrenBuilder::new()
        .with_module("main", |m| {
            m.register::<WrenEngine>();
        })
        .build();

    // Class must be declared in both Rust and Wren
    vm.interpret("main", DECLARE_ENGINE).expect("Foreign class declaration");

    vm.interpret("game_logic", r#"
    import "main" for Engine

    var engine = Engine.new()
    System.print("Engine version %(engine.version)")
    System.print("Add -> %(engine.add(30, 12))")
    "#).expect("Game logic");
}
```

## Safety

This library aims to be as safe as is reasonable for a Rust project embedding a C library.
If usage of any function not marked as `unsafe` results in undefined behaviour, then it's a bug.

However, complete safety is not possible because Wren itself is not safe. As a deliberate
design choice of the embedding API, Wren prioritises performance and leaves safety checks
to the embedding code.

Lists can be accessed without bounds checks. Retrieving a variable does not validate whether
the module or variable exists. The lifetimes of strings involved in certain calls are unclear.
Sometimes this results in a SEGFAULT, but mostly the VM silently dereferences junk pointers.

The following precautions have been taken in the design of this library to mitigate these issues:

- Foreign classes declared in Rust but owned by Wren are wrapped in `RefCell<T>` to perform
  borrow checking at runtime. There's nothing stopping Wren from sending the same instance
  to a foreign function in multiple arguments.
  
  ```dart
  var obj = GameObject.new()
  Game.doBattle(obj, obj)
  ```
  
  ```rust
  #[method(name = doBattle)]
  fn do_battle(attack: &WrenCell<GameObject>, defend: &WrenCell<GameObject>) {
      attack.borrow_mut().unwrap();
  
      // If both cells refer to the same
      // value, the second borrow will fail.
      defend.borrow_mut().unwrap();
  }
  ```
- Access to the VM's [slots](https://wren.io/embedding/slots-and-handles.html) does type- and bounds checks.
- Handles must be released before the VM is dropped. Safe access to handles is done via the `WrenContext`
  in a closure, which drops any created handles when the closure returns. Handles with the `'static` lifetime
  can be created via an `unsafe` interface.

## Limitations

Wren is still in development, and lacks certain features in its embedding API.

- There is no way to declare a module level Wren variable from Rust, or directly change the value of
  an existing module variable. Workarounds are calling methods on classes that mutate the variable or
  setting the fields of a `Map`.
- Instantiating a foreign class from Wren will call the factory function supplied via `rust-wren`, then
  call the `construct` method on the Wren side. However, creating the same class (struct) in Rust and
  passing ownership to Wren will not call its constructor. There is no way to call a Wren constructor
  without triggering allocation. See the [example](./examples/issue_construct.rs).
  
  ```rust
  #[method(name = createInstance)]
  fn create_instance(id: i32) -> GameObject {
      // The `construct` method declared in Wren will not be called.
      GameObject::new(id)
  }
  ```
- Lists can insert and set elements, but there is no remove.
- [Fixed] Looking up a variable via `WrenContext::var_ref`, with either the module or variable not existing, is
  undefined behaviour. Fixed by : https://github.com/wren-lang/wren/pull/717

## TODO

- [x] Lookup for foreign methods must take `is_static` into account.
- [x] Generate a `ToWren` implementation for each `WrenForeignClass`. Requires lookup of class variable, and is mostly the same as `__wren_allocate`.
- [x] Methods must handle arguments that implement `WrenForeignClass`, but are not the receiver. 
- [x] Implement properties
- [ ] Permit `construct` method to be omitted; generate `__wren_allocate` using `WrenForeignClass::default`
- [ ] Store foreign method bindings in `inventory`.
- [ ] Wren operator methods.
- [ ] Non-static userdata borrowed within scope.
- [ ] Replaced raw pointers with NonNull

# License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
