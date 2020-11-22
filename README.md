
## Build

### Windows

```
msbuild projects\vs2019\wren.vcxproj /property:Configuration="Debug 64bit" /property:Platform=x64
```

## Notes

Ideas for procedural macros.

```rust
#[wren_class(name="Engine")]
struct WrenEngine {
    version: u32,
}

#[wren_methods]
impl WrenEngine {
    #[construct]
    fn new() -> Self {
        WrenEngine { version: 1 }
    }

    fn version(&self) -> WrenResult<u32> {
        self.version
    }
}

#[wren_class(name="Vector2")]
struct WrenVector2(nalgebra::Vector2<f64>);

#[wren_methods]
impl WrenVector {
    #[construct]
    fn new(x: f64, y: f64) -> Self {
        WrenVector2(nalgebra::Vector2::new(x, y))
    }

    #[staticmethod]
    fn zero() -> Self {
        WrenVector2(nalgebra::Vector2::new(0.0, 0.0))
    }

    #[getter]
    fn x(&self) -> f64 {
        self.x
    }

    #[setter]
    fn set_x(&self, value: f64) -> f64 {
        self.x = value;
    }
}

impl WrenSequenceProtocol for WrenVector2 {
    
}
```

## TODO

- [ ] Lookup for foreign methods must take `is_static` into account.
- [ ] Generate a `ToWren` implementation for each `WrenForeignClass`. Requires lookup of class variable, and is mostly the same as `__wren_allocate`.
- [ ] Methods must handle arguments that implement `WrenForeignClass`, but are not the receiver.
- [ ] Store foreign method bindings in `inventory`.
- [ ] Wren operator methods.
- [ ] Non-static userdata borrowed within scope.