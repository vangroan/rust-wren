use crate::{
    bindings,
    value::{FromWren, ToWren},
    vm::WrenContext,
};
use std::{borrow::Cow, ffi::CString, marker::PhantomData, sync::mpsc::Sender};

/// Temporary borrow of a Wren handle.
pub struct WrenRef<'wren> {
    handle: *mut bindings::WrenHandle,
    destructors: Sender<*mut bindings::WrenHandle>,
    _marker: PhantomData<&'wren bindings::WrenHandle>,
}

impl<'wren> WrenRef<'wren> {
    pub(crate) fn new(
        handle: &mut bindings::WrenHandle,
        destructors: Sender<*mut bindings::WrenHandle>,
    ) -> Self {
        WrenRef {
            handle,
            destructors,
            _marker: PhantomData,
        }
    }
}

impl<'wren> Drop for WrenRef<'wren> {
    fn drop(&mut self) {
        println!("Dropping WrenRef {:?}", self.handle);
        self.destructors
            .send(self.handle)
            .unwrap_or_else(|err| eprintln!("{}", err));
    }
}

impl<'wren> FromWren<'wren> for WrenRef<'wren> {
    type Output = Self;

    fn get_slot(ctx: &mut WrenContext, slot_num: i32) -> Option<Self::Output> {
        let handle = unsafe {
            bindings::wrenGetSlotHandle(ctx.vm, slot_num)
                .as_mut()
                .unwrap()
        };
        let destructors = ctx.destructor_sender();
        Some(WrenRef::new(handle, destructors))
    }
}

impl<'wren> ToWren for WrenRef<'wren> {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        unsafe {
            bindings::wrenSetSlotHandle(ctx.vm, slot, self.handle);
        }
    }
}

pub struct FnSymbol<'wren> {
    handle: WrenRef<'wren>,
}

impl<'wren> FnSymbol<'wren> {
    pub fn compile<'a, S>(ctx: &mut WrenContext, signature: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        let sig_c = CString::new(signature.into().as_ref())
            .expect("Function signature contained a null byte");
        let handle = unsafe {
            bindings::wrenMakeCallHandle(ctx.vm, sig_c.as_ptr())
                .as_mut()
                .unwrap()
        };
        let destructors = ctx.destructor_sender();
        FnSymbol {
            handle: WrenRef::new(handle, destructors),
        }
    }
}

/// Reference to a receiver variable and function symbol within
/// a [`WrenVm`](../struct.WrenVm.html).
///
/// Tied to the lifetime of a context scope.
///
/// Used to call Wren methods from Rust.
///
/// # Example
///
/// ```
/// # use rust_wren::prelude::*;
/// # let mut vm = WrenBuilder::new().build();
/// use rust_wren::handle::{FnSymbol, WrenCallRef};
///
/// vm.interpret("example", r#"
///     class Factorial {
///         static recursive(n) {
///             if (n < 2) return 1
///             return n * recursive(n-1)
///         }
///     }
/// "#).unwrap();
///
/// vm.context(|ctx| {
///     let receiver = ctx.get_var("example", "Factorial").unwrap();
///     let func = FnSymbol::compile(ctx, "recursive(_)");
///     let call_ref = WrenCallRef::new(receiver, func);
///     
///     let result = call_ref.call::<_, f64>(ctx, 7.0).unwrap();
///     # assert_eq!(result, 5040.0);
/// });
/// ```
pub struct WrenCallRef<'wren> {
    receiver: WrenRef<'wren>,
    func: FnSymbol<'wren>,
}

impl<'wren> WrenCallRef<'wren> {
    pub fn new(receiver: WrenRef<'wren>, func: FnSymbol<'wren>) -> Self {
        Self { receiver, func }
    }

    /// Call Wren method.
    ///
    /// The argument is any value that implements [ToWren](../value/trait.ToWren.html).
    /// A tuple struct can be used to pass multiple arguments. Because the values will
    /// be sent to Wren, they will be moved, or implicitly copied.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_wren::prelude::*;
    /// # let mut vm = WrenBuilder::new().build();
    /// use rust_wren::handle::{FnSymbol, WrenCallRef};
    /// vm.interpret("example", r#"
    ///     class Calculate {
    ///         static addOrSub(a, b, c) {
    ///             if (c) {
    ///                 return a + b
    ///             } else {
    ///                 return a - b
    ///             }
    ///         }
    ///     }
    /// "#).unwrap();
    ///
    /// vm.context(|ctx| {
    ///     let receiver = ctx.get_var("example", "Calculate").unwrap();
    ///     let func = FnSymbol::compile(ctx, "addOrSub(_,_,_)");
    ///     let call_ref = WrenCallRef::new(receiver, func);
    ///     
    ///     let result = call_ref.call::<(f64, f64, bool), f64>(ctx, (4.0, 6.0, true)).unwrap();
    ///     # assert_eq!(result, 10.0);
    ///     let result = call_ref.call::<(f64, f64, bool), f64>(ctx, (4.0, 6.0, false)).unwrap();
    ///     # assert_eq!(result, -2.0);
    /// });
    /// ```
    pub fn call<'a, A, R>(&self, ctx: &'a mut WrenContext, args: A) -> Option<R::Output>
    where
        A: ToWren,
        R: FromWren<'a>,
    {
        // Receiver and arguments.
        ctx.ensure_slots(1 + args.size_hint());
        // FIXME: Move. We also don't want to copy WrenHandle
        // self.receiver.put(ctx, 0);
        println!("Set slot receiver {:?}", self.receiver.handle);
        unsafe {
            bindings::wrenSetSlotHandle(ctx.vm, 0, self.receiver.handle);
        }

        args.put(ctx, 1);

        println!("wrenCall {:?}", self.func.handle.handle);
        let _result = unsafe { bindings::wrenCall(ctx.vm, self.func.handle.handle) };

        // TODO: Check result
        R::get_slot(ctx, 0)
    }
}
