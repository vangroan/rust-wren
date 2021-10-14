//! Handles to values stored in Wren.
//!
//! A handle wraps a pointer to a value stored in Wren, which can be of any type. They keep the referenced value from
//! being garbage collected. Handles can be created by looking up a variable using the context.
//!
//! ```
//! # use rust_wren::prelude::*;
//! # let mut vm = WrenBuilder::new().build();
//! # vm.interpret("my_module", r#"var myVariable = 1234"#).expect("Interpret failed");
//! vm.context(|ctx| {
//!     let my_handle = ctx.get_var("my_module", "myVariable").unwrap();
//! });
//! ```
//!
//! By default the retrieved handle is tied to the lifetime of the context closure. This is to ensure that the handles
//! are dropped when the context scope ends, triggering a release in the Wren VM instance. Without releasing the handle,
//! the value it's referencing would not be garbage collected.
//!
//! ```compile_fail
//! # use rust_wren::{prelude::*, handle::*};
//! # let mut vm = WrenBuilder::new().build();
//! # vm.interpret("my_module", r#"var myVariable = 1234"#).expect("Interpret failed");
//! let mut handle: Option<WrenRef> = None;
//! vm.context(|ctx| {
//!     handle = ctx.get_var("my_module", "myVariable");
//! });
//! ```
//!
//! A borrowed handle can be converted to an owned handle by leaking it. From there it can be passed around like any
//! value, and triggers the handle release when it's dropped.
//!
//! ```
//! # use rust_wren::prelude::*;
//! # let mut vm = WrenBuilder::new().build();
//! # vm.interpret("my_module", r#"var myVariable = 1234"#).expect("Interpret failed");
//! use rust_wren::handle::WrenHandle;
//! let mut handle = vm.context_result(|ctx| {
//!     ctx.get_var("my_module", "myVariable")?.leak()
//! }).unwrap();
//! ```
//!
//! **Important:** If the owned handle outlives the VM, as in the VM is dropped before the handle is dropped and
//! released, the program will exit in debug mode and do nothing in release mode. A proper panic is to-be-implemented.
//!
//! ```no_run
//! # use rust_wren::prelude::*;
//! # let mut vm = WrenBuilder::new().build();
//! # vm.interpret("my_module", r#"var myVariable = 1234"#).expect("Interpret failed");
//! use rust_wren::handle::WrenHandle;
//! let mut handle = vm.context_result(|ctx| {
//!     ctx.get_var("my_module", "myVariable")?.leak()
//! }).unwrap();
//! drop(vm); // <-- processes exit
//! ```
//!
//! The borrowed and owned flavours for handles are:
//!
//! - [`WrenRef`](struct.WrenRef.html) - Borrowed handle to a variable that's scoped to a [`WrenVm::context`](../struct.WrenVm.html#method.context) closure.
//! - [`FnSymbolRef`](struct.FnSymbol.html) - Borrowed handle to a compiled function signature.
//! - [`WrenCallRef`](struct.WrenCallRef.html) - Borrowed call handle for calling methods in Wren.
//! - [`WrenHandle`](struct.WrenHandle.html) - Owned handle to a variable that can be stored outside a context scope.
//! - [`FnSymbol`](struct.FnSymbol.html) - Owned handle to a compiled function signature that can be stored outside a contex scope.
//! - [`WrenCallHandle`](struct.WrenCallHandle.html) - Owned call handle that can be stored outside a context scope.
//!
//! # Examples
//!
//! Variables can be retrieved via the context, returning a [`WrenRef`](struct.WrenRef.html).
//!
//! ```
//! # use rust_wren::{prelude::*, handle::*};
//! # #[wren_class]
//! # struct MyForeignClass;
//! # #[wren_methods]
//! # impl MyForeignClass { #[construct] fn new() -> Self { Self } }
//! # let mut vm = WrenBuilder::new().with_module("example", |m| m.register::<MyForeignClass>() ).build();
//! vm.interpret("example", r#"
//! var str = "String"
//! var boo = true
//! var num = 11
//! var lst = [1, 2, 3]
//! var map = {"a": 1, "b": 2, "c": 3}
//! var nil = null
//! var fib = Fiber.current
//! var fun = Fn.new { "Hello, World!" }
//!
//! class MyClass {}
//!
//! foreign class MyForeignClass {
//!     construct new() {}
//! }
//! var obj = MyForeignClass.new()
//! "#).expect("Interpret failed");
//!
//! vm.context(|ctx| {
//!     let var_str: WrenRef = ctx.get_var("example", "str").unwrap();
//!     let var_boo: WrenRef = ctx.get_var("example", "boo").unwrap();
//!     let var_num: WrenRef = ctx.get_var("example", "num").unwrap();
//!     let var_lst: WrenRef = ctx.get_var("example", "lst").unwrap();
//!     let var_map: WrenRef = ctx.get_var("example", "map").unwrap();
//!     let var_nil: WrenRef = ctx.get_var("example", "nil").unwrap();
//!     let var_fib: WrenRef = ctx.get_var("example", "fib").unwrap();
//!     let var_fun: WrenRef = ctx.get_var("example", "fun").unwrap();
//!     let var_obj: WrenRef = ctx.get_var("example", "obj").unwrap();
//!
//!     // Class definitions can be retrieved like variables.
//!     let var_cls: WrenRef = ctx.get_var("example", "MyClass").unwrap();
//!     let var_for: WrenRef = ctx.get_var("example", "MyForeignClass").unwrap();
//!
//!     // Handles are released via the VM at the end of the closure.
//! });
//! ```
//!
//! Calls to Wren are made via handles.
//!
//! ```
//! # use rust_wren::prelude::*;
//! # let mut vm = WrenBuilder::new().build();
//! use rust_wren::handle::{FnSymbolRef, WrenCallRef};
//!
//! vm.interpret("example", r#"
//! class MyClass {
//!     construct new() {}
//!     static sayStatic(msg) { "Static method says '%(msg)'" }
//!     sayInstance(msg) { "Instance method says '%(msg)'" }
//! }
//!
//! var obj = MyClass.new()
//! var message = "Hello from Wren"
//! "#).expect("Interpret failed");
//!
//! vm.context(|ctx| {
//!     // For static method calls, the class itself is the receiver.
//!     let cls_receiver = ctx.get_var("example", "MyClass").unwrap();
//!
//!     // Compile the function signature and return a `WrenRef` to it.
//!     let fn_static = FnSymbolRef::compile(ctx, "sayStatic(_)").unwrap();
//!
//!     // Combine the receiver and function symbol into a call reference.
//!     let call_static = WrenCallRef::new(cls_receiver, fn_static);
//!
//!     // Method can now be called, and take Rust values as arguments.
//!     let rust_msg = call_static.call::<_, String>(ctx, "Hello from Rust").unwrap();
//!     assert_eq!(rust_msg.as_str(), "Static method says 'Hello from Rust'");
//!
//!     // Handles to variables can be used as call arguments.
//!     let var_hello = ctx.get_var("example", "message").unwrap();
//!     let wren_msg = call_static.call::<_, String>(ctx, var_hello).unwrap();
//!     assert_eq!(wren_msg.as_str(), "Static method says 'Hello from Wren'");
//!
//!     // Instance methods work the same, except the receiver is an instance of an object.
//!     let obj_receiver = ctx.get_var("example", "obj").unwrap();
//!     let fn_instance = FnSymbolRef::compile(ctx, "sayInstance(_)").unwrap();
//!     let call_instance = WrenCallRef::new(obj_receiver, fn_instance);
//!     let instance_msg = call_instance.call::<_, String>(ctx, "Hello from Rust").unwrap();
//!     assert_eq!(instance_msg.as_str(), "Instance method says 'Hello from Rust'");
//!
//!     // The context has a helper function for creating a call reference.
//!     let call_instance: WrenCallRef = ctx.make_call_ref("example", "obj", "sayInstance(_)").unwrap();
//! });
//! ```
//!
//! Passing a variable's handle to Wren for a call consumes the handle. To keep the handle
//! and pass it multiple times, pass a reference.
//!
//! ```
//! # use rust_wren::{prelude::*, handle::*};
//! # let mut vm = WrenBuilder::new().build();
//!
//! vm.interpret("example", r#"
//! class Example {
//!     static multiply(lhs, rhs) { lhs * rhs }
//! }
//!
//! var a = 4
//! "#).expect("Interpret failed");
//!
//! vm.context(|ctx| {
//!     let call_ref = ctx.make_call_ref("example", "Example", "multiply(_,_)").unwrap();
//!     let var_a = ctx.get_var("example", "a").unwrap();
//!
//!     assert_eq!(call_ref.call::<_, f64>(ctx, (&var_a, 2.0)).ok(), Some(8.0));
//!     assert_eq!(call_ref.call::<_, f64>(ctx, (&var_a, 3.0)).ok(), Some(12.0));
//!     assert_eq!(call_ref.call::<_, f64>(ctx, (&var_a, 4.0)).ok(), Some(16.0));
//! });
//! ```
use crate::{
    bindings,
    errors::{WrenError, WrenResult},
    value::{FromWren, ToWren},
    vm::WrenContext,
};
use regex::Regex;
use std::{
    borrow::Cow,
    ffi::CString,
    fmt,
    marker::PhantomData,
    mem,
    ptr::NonNull,
    rc::Rc,
    sync::{mpsc::Sender, Arc},
};

/// Borrowed handle to a variable that's scoped to a [`WrenVm::context`](../struct.WrenVm.html#method.context) closure.
pub struct WrenRef<'wren> {
    handle: *mut bindings::WrenHandle,
    destructors: Option<Sender<*mut bindings::WrenHandle>>,
    _marker: PhantomData<&'wren bindings::WrenHandle>,
}

impl<'wren> WrenRef<'wren> {
    pub(crate) fn new(handle: &mut bindings::WrenHandle, destructors: Sender<*mut bindings::WrenHandle>) -> Self {
        WrenRef {
            handle,
            destructors: Some(destructors),
            _marker: PhantomData,
        }
    }

    /// Convert this borrowed `WrenRef` into an owned [`WrenHandle`](struct.WrenHandle.html).
    ///
    /// # Errors
    ///
    /// Returns [`WrenError::AlreadyLeaked`](../errors/enum.WrenError.html#variant.AlreadyLeaked) if the internal state
    /// of the handle indicates that it has already been leaked.
    pub fn leak(mut self) -> WrenResult<WrenHandle> {
        let WrenRef {
            handle,
            ref mut destructors,
            ..
        } = self;

        // We cannot move fields out of self, because its lifetime
        // and marker make it appear that it is borrowing a value and
        // doesn't own its contents.
        //
        // We shouldn't mem::forget the sender because internally it is
        // an Arc that needs its counter eventually decremented.
        let destructors = destructors.take().ok_or(WrenError::AlreadyLeaked)?;

        // SAFETY: Ownership of the internal handle and channel sender moves to the new
        //         struct, where it will be responsibly dropped and released. However if we
        //         were to drop the WrenRef, the handle will experience a double-free within the
        //         Wren VM.
        mem::forget(self);

        Ok(WrenHandle { handle, destructors })
    }
}

impl<'wren> fmt::Debug for WrenRef<'wren> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WrenRef").field("handle", &self.handle).finish()
    }
}

impl<'wren> Drop for WrenRef<'wren> {
    fn drop(&mut self) {
        log::trace!("Dropping WrenRef {:?}", self.handle);
        if let Some(d) = self.destructors.take() {
            d.send(self.handle).unwrap_or_else(|err| eprintln!("{}", err));
        }
    }
}

impl<'wren> FromWren<'wren> for WrenRef<'wren> {
    type Output = Self;

    fn get_slot(ctx: &WrenContext, slot_num: i32) -> WrenResult<Self::Output> {
        let handle = unsafe { bindings::wrenGetSlotHandle(ctx.vm_ptr(), slot_num).as_mut().unwrap() };
        let destructors = ctx.destructor_sender();
        Ok(WrenRef::new(handle, destructors))
    }
}

impl<'wren> ToWren for WrenRef<'wren> {
    #[inline]
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        <&WrenRef>::put(&self, ctx, slot);
    }
}

impl<'wren> ToWren for &WrenRef<'wren> {
    #[inline]
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        unsafe {
            bindings::wrenSetSlotHandle(ctx.vm_ptr(), slot, self.handle);
        }
    }
}

/// Borrowed handle to a compiled function signature that's scoped to a [`WrenVm::context`](../struct.WrenVm.html#method.context).
pub struct FnSymbolRef<'wren> {
    handle: WrenRef<'wren>,
}

impl<'wren> FnSymbolRef<'wren> {
    /// Regex pattern for validating function signatures.
    const SIG_PATTERN: &'static str = r#"^[a-zA-Z0-9_]+(\(([_,]*[^,])?\))$"#;

    pub fn compile<'a, S>(ctx: &WrenContext, signature: S) -> WrenResult<Self>
    where
        S: Into<Cow<'a, str>>,
    {
        lazy_static! {
            static ref RE: Regex = Regex::new(FnSymbolRef::SIG_PATTERN).unwrap();
        }
        let sig_cow = signature.into();
        let sig = sig_cow.as_ref();
        // FIXME: Regex not enough to validate function signature, because of properties and operators.
        // if !RE.is_match(sig) {
        //     println!("Invalid function signature {}", sig);
        //     return None;
        // }

        let sig_c = CString::new(sig).expect("Function signature contained a null byte");
        let handle = unsafe {
            bindings::wrenMakeCallHandle(ctx.vm_ptr(), sig_c.as_ptr())
                .as_mut()
                .unwrap()
        };
        let destructors = ctx.destructor_sender();

        Ok(FnSymbolRef {
            handle: WrenRef::new(handle, destructors),
        })
    }

    /// Convert the borrowed `FnSymbolRef` into an owned [`FnSymbol`](struct.FnSymbol.html).
    ///
    /// # Safety
    ///
    /// By detaching the internal handle from the context lifetime, there
    /// is a risk of freeing the VM before releasing this handle.
    ///
    /// You take responsibility for making sure this is dropped before
    /// the VM is dropped.
    pub fn leak(self) -> WrenResult<FnSymbol> {
        let FnSymbolRef { handle } = self;

        handle.leak().map(|handle| FnSymbol { handle })
    }
}

/// Borrowed call handle for calling methods in Wren, scoped to a [`WrenVm::context`](../struct.WrenVm.html#method.context) closure.
///
/// Combines a receiver variable and function symbol for convenience.
///
/// Used to call Wren methods from Rust.
///
/// # Example
///
/// ```
/// # use rust_wren::prelude::*;
/// # let mut vm = WrenBuilder::new().build();
/// use rust_wren::handle::{FnSymbolRef, WrenCallRef};
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
///     let func = FnSymbolRef::compile(ctx, "recursive(_)").unwrap();
///     let call_ref = WrenCallRef::new(receiver, func);
///     
///     let result = call_ref.call::<_, f64>(ctx, 7.0).unwrap();
///     # assert_eq!(result, 5040.0);
/// });
/// ```
pub struct WrenCallRef<'wren> {
    receiver: WrenRef<'wren>,
    func: FnSymbolRef<'wren>,
}

impl<'wren> WrenCallRef<'wren> {
    pub fn new(receiver: WrenRef<'wren>, func: FnSymbolRef<'wren>) -> Self {
        Self { receiver, func }
    }

    /// Call Wren method.
    ///
    /// The argument is any value that implements [`ToWren`](../value/trait.ToWren.html).
    /// A tuple struct can be used to pass multiple arguments. Because the values will
    /// be sent to Wren, they will be moved, or implicitly copied.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rust_wren::prelude::*;
    /// # let mut vm = WrenBuilder::new().build();
    /// use rust_wren::handle::{FnSymbolRef, WrenCallRef};
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
    ///     let func = FnSymbolRef::compile(ctx, "addOrSub(_,_,_)").unwrap();
    ///     let call_ref = WrenCallRef::new(receiver, func);
    ///     
    ///     let result = call_ref.call::<(f64, f64, bool), f64>(ctx, (4.0, 6.0, true)).unwrap();
    ///     # assert_eq!(result, 10.0);
    ///     let result = call_ref.call::<(f64, f64, bool), f64>(ctx, (4.0, 6.0, false)).unwrap();
    ///     # assert_eq!(result, -2.0);
    /// });
    /// ```
    pub fn call<'ctx, A, R>(&self, ctx: &'ctx mut WrenContext, args: A) -> WrenResult<R::Output>
    where
        A: ToWren,
        R: FromWren<'wren>,
    {
        let receiver = unsafe { self.receiver.handle.as_mut().ok_or(WrenError::NullPtr)? };
        let func = unsafe { self.func.handle.handle.as_mut().ok_or(WrenError::NullPtr)? };

        wren_call::<A, R>(ctx, receiver, func, args)
    }

    pub fn leak(self) -> WrenResult<WrenCallHandle> {
        let WrenCallRef { receiver, func } = self;

        if let (Ok(receiver), Ok(func)) = (receiver.leak(), func.leak()) {
            Ok(WrenCallHandle { receiver, func })
        } else {
            Err(WrenError::AlreadyLeaked)
        }
    }
}

/// Owned handle to a variable stored in Wren.
///
/// The handle does not implement [`FromWren`](../value/trait.FromWren.html). To accept a handle
/// in a foreign method, use [`WrenRef`](struct.WrenRef.html) and explicitly leak it. This
/// is a deliberate design choice so the library user is aware they take responsibility
/// for dropping the handle before dropping the VM.
pub struct WrenHandle {
    handle: *mut bindings::WrenHandle,
    destructors: Sender<*mut bindings::WrenHandle>,
}

/// Our `WrenHandle` wrapper is designed to be only useful with the VM they belong to. The user can't use
/// the raw pointer to retrieve the data, write to it, or cause race conditions.
///
/// Sending the pointer across thread boundaries should be safe, and is needed to implement concurrency scheduling.
unsafe impl Send for WrenHandle {}

impl WrenHandle {
    /// Create a new `WrenHandle` from a raw pointer.
    ///
    /// TODO: Change usage to NonNull to ensure internal integrity of `WrenHandle`
    pub(crate) unsafe fn from_raw(
        handle: *mut bindings::WrenHandle,
        destructors: Sender<*mut bindings::WrenHandle>,
    ) -> Self {
        WrenHandle { handle, destructors }
    }

    /// Retrieve the raw underlying pointer.
    #[inline(always)]
    pub(crate) unsafe fn raw_ptr(&self) -> NonNull<bindings::WrenHandle> {
        // FIXME: WrenHandle internally must be NonNull to begin with
        NonNull::new_unchecked(self.handle)
    }
}

impl fmt::Debug for WrenHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WrenHandle").field("handle", &self.handle).finish()
    }
}

impl Drop for WrenHandle {
    fn drop(&mut self) {
        log::trace!("Dropping {:?}", self.handle);
        self.destructors
            .send(self.handle)
            .unwrap_or_else(|err| eprintln!("{}", err));
    }
}

impl ToWren for WrenHandle {
    #[inline]
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        <&WrenHandle>::put(&self, ctx, slot);
    }
}

impl ToWren for &WrenHandle {
    #[inline]
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        unsafe {
            bindings::wrenSetSlotHandle(ctx.vm_ptr(), slot, self.handle);
        }
    }
}

/// Allows a wrapped [`WrenHandle`](struct.WrenHandle.html) to be passed to Wren methods with minimum fuss.
impl ToWren for Rc<WrenHandle> {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        unsafe {
            bindings::wrenSetSlotHandle(ctx.vm_ptr(), slot, self.handle);
        }
    }
}

/// Allows a wrapped [`WrenHandle`](struct.WrenHandle.html) to be passed to Wren methods with minimum fuss.
impl ToWren for Arc<WrenHandle> {
    fn put(self, ctx: &mut WrenContext, slot: i32) {
        unsafe {
            bindings::wrenSetSlotHandle(ctx.vm_ptr(), slot, self.handle);
        }
    }
}

/// Owned handle to a compiled function signature stored in Wren.
///
/// Create by leaking a [`FnSymbolRef`](struct.FnSymbolRef.html).
pub struct FnSymbol {
    handle: WrenHandle,
}

/// Owned call handle for calling methods in Wren.
///
/// Combines a receiver variable and function symbol for convenience.
///
/// Used to call Wren methods from Rust.
pub struct WrenCallHandle {
    receiver: WrenHandle,
    func: FnSymbol,
}

impl WrenCallHandle {
    pub fn call<'wren, 'ctx, A, R>(&self, ctx: &'ctx mut WrenContext, args: A) -> WrenResult<R::Output>
    where
        A: ToWren,
        R: FromWren<'wren>,
    {
        let receiver = unsafe { self.receiver.handle.as_mut().ok_or(WrenError::NullPtr)? };
        let func = unsafe { self.func.handle.handle.as_mut().ok_or(WrenError::NullPtr)? };

        wren_call::<A, R>(ctx, receiver, func, args)
    }
}

/// Perform Wren function call.
fn wren_call<'wren, 'ctx, A, R>(
    ctx: &'ctx mut WrenContext,
    receiver: &mut bindings::WrenHandle,
    func: &mut bindings::WrenHandle,
    args: A,
) -> WrenResult<R::Output>
where
    A: ToWren,
    R: FromWren<'wren>,
{
    // Receiver and arguments.
    ctx.ensure_slots(1 + args.size_hint());

    // FIXME: WrenHandle is moved via ToWren.
    //        It shouldn't be clone because that would require us to
    //        wrap it `Rc<T>` and introduce even more indirection.
    //        Create `WrenHandle::clone(ctx)`.
    unsafe {
        bindings::wrenSetSlotHandle(ctx.vm_ptr(), 0, receiver);
    }

    args.put(ctx, 1);

    let result_id: bindings::WrenInterpretResult = unsafe { bindings::wrenCall(ctx.vm_ptr(), func) };
    ctx.take_errors(result_id)?;

    // Wren places the result in slot 0 if result was success.
    R::get_slot(ctx, 0)
}
