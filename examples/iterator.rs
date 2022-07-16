//! Demonstration of the iterator protocol using a foreign class.
//!
//! Wren Documentation: https://wren.io/control-flow.html#the-iterator-protocol
//!
//! When looping a collection such as a list or sequence in Wren,
//! there is an implicit iterator protocol which the target iterated
//! class must implement.
//!
//! Because of duck-typing the target class can simply implement methods
//! with the correct signatures to satify the protocol.
//!
//! Given a `for` loop like this:
//!
//! ```non-rust
//! for (i in a) {
//!     System.print(i)
//! }
//! ```
//!
//! Wren does something like this behind the scenes:
//!
//! ```non-rust
//! var iter_ = null
//! var seq_ = a
//! while (iter_ = seq_.iterate(iter_)) {
//!   var i = seq_.iteratorValue(iter_)
//!   System.print(i)
//! }
//! ```
//!
//! The expression `a` is evaluated and stored in a hidden variable `seq_`.
//! There is also a hidden variable `iter_` which will act as the cursor
//! that stores the state of each iteration index. It is both input and
//! output of the iteration interface that `a` must implement.
//!
//! Our iterator must implement these two methods:
//!
//! - `iterate(index: Null | Num) -> Bool | Num`
//!   Controls the flow of the loop.
//!   Receives `null` on the first iteration of the loop,
//!   or the last index state returned by a previous invocation.
//!   Returns the next index state if iteration should continue,
//!   or `false` when iteration should stop.
//! - `iteratorValue(index: Num) -> Null | T`
//!   Accessor to the current value the index state is pointing to.
//!
//! The initial value of `iter_` is `null` to indicate the start of iteration.
use rust_wren::prelude::*;
use rust_wren::WrenContext;

/// Array of f64.
///
/// This is our iterable collection class.
///
/// Note that the class itself does not store any iteration state.
/// We expose an interface that satisfies the iterator protocol,
/// and Wren will store whatever index value we give it on its
/// callstack.
///
/// When Wren does a loop iteration, it will hand our last
/// index value back, then we need to tell it what to do next.
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
    fn set_data(&mut self, #[ctx] ctx: &mut WrenContext, data: WrenList) -> rust_wren::Result<()> {
        self.data = data.to_vec::<f64>(ctx).map_err(|err| foreign_error!(err))?;

        Ok(())
    }

    /// Control flow.
    fn iterate(&self, maybe_index: Option<u32>) -> WrenIter {
        match maybe_index {
            None => {
                // Initially the iterator value is null
                if self.data.is_empty() {
                    WrenIter::Done
                } else {
                    WrenIter::Continue(0) // first element index
                }
            }
            Some(mut index) => {
                // Value passed to Wren in previous iteration.
                index += 1;
                if index as usize >= self.data.len() {
                    WrenIter::Done
                } else {
                    WrenIter::Continue(index)
                }
            }
        }
    }

    /// The current value in our collection that the iterator cursor is pointing to.
    #[method(name = iteratorValue)]
    fn iterator_value(&self, index: u32) -> Option<f64> {
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

/// Control flow instructions for Wren.
///
/// An adapter like this makes for a more readable iterator
/// on the Rust side. An enum is also required to pick
/// between `Num` and `Bool` types.
///
/// When it crosses the boundary to Wren it becomes the
/// the integer or boolean as per the iterator protocol
/// to drive the loop.
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
