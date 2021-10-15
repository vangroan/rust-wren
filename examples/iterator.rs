//! Demonstration of the iterator protocol using a foreign class.
use rust_wren::prelude::*;
use rust_wren::{WrenContext, WrenError, WrenResult};

/// Array of f64
#[wren_class]
struct Array {
    data: Vec<f64>,
}

#[wren_methods]
impl Array {
    #[construct]
    fn new() -> Self {
        Array { data: vec![] }
    }

    #[method(name=setData)]
    fn set_data(&mut self, data: Vec<f64>) {
        self.data = data;
    }

    fn iterate(&self, maybe_index: Option<u32>) -> WrenIter {
        match maybe_index {
            None => {
                // Initially the iterator value is null
                if self.data.is_empty() {
                    WrenIter::Done
                } else {
                    WrenIter::Continue(0)
                }
            }
            Some(mut index) => {
                // Value passed to Wren in
                // previous iteration.
                index += 1;
                if index as usize >= self.data.len() {
                    WrenIter::Done
                } else {
                    WrenIter::Continue(index)
                }
            }
        }
    }

    fn iteratorValue(&self, index: u32) -> Option<f64> {
        self.data.get(index as usize).cloned()
    }
}

const DECLARE_SCRIPT: &str = r#"
foreign class Array {
  construct new() {}
  foreign setData(data)
  foreign iterate(iter)
  foreign iteratorValue(iter)
}
"#;

enum WrenIter {
    Continue(u32),
    Done,
}

impl ToWren for WrenIter {
    fn put(self, ctx: &mut WrenContext, slot_num: i32) {
        ctx.ensure_slots(slot_num as usize + 1);

        match self {
            Self::Continue(idx) => ToWren::put(idx, ctx, slot_num),
            Self::Done => ToWren::put(false, ctx, slot_num),
        }
    }
}

fn main() {
    let mut vm = WrenBuilder::new()
        .with_module("main", |m| m.register::<Array>())
        .build();
    vm.interpret("main", DECLARE_SCRIPT).unwrap();

    vm.interpret(
        "example",
        r#"
    import "main" for Array

    var a = Array.new()
    a.setData([1, 2, 3, 4, 5, 6, 7, 8, 9])

    System.print("for loop")
    for (i in a) {
        System.print(i)
    }

    System.print("iterator protocol")
    var iter_ = null
    var seq_ = a
    while (iter_ = seq_.iterate(iter_)) {
      var i = seq_.iteratorValue(iter_)
      System.print(i)
    }
    "#,
    )
    .unwrap();
}
