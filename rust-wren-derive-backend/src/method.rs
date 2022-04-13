use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Attribute, Expr, ExprAssign, FnArg, Ident, ImplItem, ImplItemMethod, ItemImpl, Lit, Signature, Token, Type,
};

pub fn build_wren_methods(mut ast: ItemImpl) -> syn::Result<TokenStream> {
    if let Some((_, path, _)) = ast.trait_ {
        Err(syn::Error::new_spanned(
            path,
            "#[wren_methods] cannot be used on trait impl blocks",
        ))
    } else if ast.generics != Default::default() {
        Err(syn::Error::new_spanned(
            ast.generics.clone(),
            "#[wren_methods] cannot be used with lifetime parameters or generics",
        ))
    } else {
        // TODO: Return ast
        let tokens = impl_methods(&ast.self_ty, &mut ast.items)?;
        // let gen = quote! { #ast };
        Ok(tokens)
    }
}

fn impl_methods(cls: &Type, impls: &mut Vec<ImplItem>) -> syn::Result<TokenStream> {
    let mut new_impl = vec![];
    let mut specs = vec![];

    for im in impls.iter_mut() {
        match im {
            ImplItem::Method(method) => {
                let (tokens, spec) = handle_method(cls, method)?;

                new_impl.push(tokens);

                // Don't add the constructor to method bindings.
                if matches!(spec.ty, WrenFnType::Method) {
                    specs.push(spec);
                }
            }
            _ => new_impl.push(quote! { #im }),
        }
    }

    let finalizer = gen_wren_finalize()?;

    let register = gen_register(&specs)?;

    // TODO: Generate register function to create function bindings for wrappers.

    let tokens = quote! {
        impl #cls {
            #(#new_impl)*

            #finalizer

            #register
        }
    };

    Ok(tokens)
}

fn handle_method(cls: &Type, method: &mut ImplItemMethod) -> syn::Result<(TokenStream, WrenFnSpec)> {
    let spec = WrenFnSpec::build(&method.sig, &mut method.attrs)?;

    // Strip attributes so we can compile.
    method.attrs.retain(|attr| !attr.path.is_ident("construct"));

    let tokens = match spec.ty {
        WrenFnType::Construct => gen_wren_construct(cls, method)?,
        WrenFnType::Method => gen_wren_method(cls, method, spec.is_static)?,
        _ => quote! { #method },
    };

    Ok((tokens, spec))
}

fn gen_wren_construct(_cls: &Type, method: &ImplItemMethod) -> syn::Result<TokenStream> {
    let new_method = method.sig.ident.clone();
    let method_name = new_method.to_string();
    let mut args = vec![];
    for (idx, arg) in method.sig.inputs.iter().enumerate() {
        // Index 0 of the construct slot would be a Wren UNKNOWN type.
        let idx_lit = Lit::new(Literal::i32_unsuffixed(idx as i32 + 1));
        match arg {
            FnArg::Receiver(_) => {
                return Err(syn::Error::new_spanned(arg, "Construct method cannot receive self"));
            }
            FnArg::Typed(arg_ty) => {
                let arg_type = arg_ty.ty.clone();
                let span = arg_type.span().clone();
                args.push(quote_spanned! {span=>
                    match ctx.get_slot::<#arg_type>(#idx_lit) {
                        Ok(value) => value,
                        Err(err) => {
                            let wren_error = rust_wren::WrenError::new_foreign_call(
                                    #method_name,
                                    Box::new(rust_wren::WrenError::GetArg { slot: #idx_lit, cause: err.into(), })
                                );

                            // `ForeignError` is our mechanism for aborting a Wren fiber
                            // with a Rust error.
                            let foreign_error = rust_wren::ForeignError::Simple(Box::new(wren_error));
                            foreign_error.put(&mut ctx, 0);

                            return;
                        }
                    }
                });
            }
        }
    }

    // Wrapped in WrenCell because the multiple pointers can be retrieved from VM.
    let ty = quote! { WrenCell<Self> };

    // Get span to function return type, so user gets a nice error when
    // the return type is incorrect.
    let method_span = method.sig.span().clone();

    let tokens = quote_spanned! {method_span=>
        #method

        /// Allocation function called by Wren when a class is constructed.
        ///
        /// Is responsible for allocating space inside the Wren VM to contain
        /// the value.
        ///
        /// See: [Storing C Data](https://wren.io/embedding/storing-c-data.html)
        #[allow(unused_mut)]
        extern "C" fn __wren_allocate(vm: *mut rust_wren::bindings::WrenVM) {
            use rust_wren::class::WrenCell;

            // Wren wants to own the memory containing the data backing the foreign function.
            let wren_ptr: *mut #ty = unsafe {
                rust_wren::bindings::wrenSetSlotNewForeign(vm, 0, 0, ::std::mem::size_of::<#ty>() as usize) as _
            };
            let wren_val: &mut #ty = unsafe { wren_ptr.as_mut().unwrap() };

            // Context for extracting slots.
            // If construct doesn't take arguments, this `vm` and `ctx` are unused.
            let vm: &mut rust_wren::bindings::WrenVM = unsafe { vm.as_mut().unwrap() };
            #[allow(unused_variables)]
            let mut ctx = rust_wren::WrenContext::new(vm);

            // TODO: Constructor method is not required, so make this optional.
            // TODO: Validate return type of constructor.
            let mut rust_val: WrenCell<Self> = WrenCell::new(<Self>::#new_method(#(#args),*));

            // Swap the constructed object on the stack with the heap memory
            // owned by Wren.
            ::std::mem::swap(wren_val, &mut rust_val);

            // After the swap, this now contains the value Wren wrote after it's allocation,
            // which is zeroed. However it's safer to treat it as undefined. Dropping a value
            // that may contain resources like boxes or file handles could cause issues if
            // it's zeroed or filled with junk.
            //
            // We're intentionally disabling drop since it wasn't initialised by Rust.
            ::std::mem::forget(rust_val);
        }
    };

    Ok(tokens)
}

fn gen_wren_finalize() -> syn::Result<TokenStream> {
    // Wrapped in WrenCell because the multiple pointers can be retrieved from VM.
    let ty = quote! { ::rust_wren::class::WrenCell<Self> };

    Ok(quote! {
        /// Finalizer method, called when the object instance is garbage collected.
        ///
        /// The VM is not available to this function, because garbage collection
        /// is in progress. Mutating the VM in the middle of gc would cause weird
        /// issues.
        ///
        /// See: [Storing C Data](https://wren.io/embedding/storing-c-data.html)
        ///
        /// # Safety
        ///
        /// Wren will deallocate the memory, obviously without calling drop. Since
        /// structs can contain resources like boxes, file handles or socket handles,
        /// it's important that the fields are properly dropped.
        ///
        /// We can't [`Box::from_raw()`] on the given `c_void` pointer, because it
        /// would take ownership of the heap data and deallocate it on drop, resulting
        /// in a double free when Wren deallocates.
        ///
        /// Instead we create a new **unsafe** zeroed object on the stack, and swap the
        /// memory values. The potentially troublesome resources (boxes, handles) will
        /// be dropped by the stack, and the Wren garbage collector will deallocate the
        /// unsafe zeroed struct.
        unsafe extern "C" fn __wren_finalize(data: *mut ::std::os::raw::c_void) {
            // This zeroed value is assumed initialised, but really it's not. Importantly
            // this value shouldn't be dropped. The drop method for the type could
            // reasonably expect valid contents.
            let mut rust_val = ::std::mem::MaybeUninit::<#ty>::zeroed().assume_init();
            let mut wren_val = (data as *mut #ty).as_mut().unwrap();

            // The unsafe zeroed memory now lives in Wren, and will be deallocated
            // without drop by the garbage collector.
            ::std::mem::swap(&mut rust_val, wren_val);

            // The contents are now the initialised value that lived in Wren's heap.
            drop(rust_val);
        }
    })
}

/// Generate a method AST.
fn gen_wren_method(_cls: &Type, method: &mut ImplItemMethod, is_static: bool) -> syn::Result<TokenStream> {
    let method_ident = method.sig.ident.clone();

    let ctx = format_ident!("ctx");
    let args = gen_args_from_slots(&ctx, method, is_static)?;

    let wrap_ident = format_ident!("__wren_wrap_{}", method.sig.ident);
    let wrap = quote! {
        #[doc(hidden)]
        extern "C" fn #wrap_ident(vm: *mut rust_wren::bindings::WrenVM) {
            // Context for extracting slots.
            let vm: &mut rust_wren::bindings::WrenVM = unsafe { vm.as_mut().unwrap() };
            let mut ctx = rust_wren::WrenContext::new(vm);

            let result = <Self>::#method_ident(#(#args),*);

            // Method result goes into slot 0
            ctx.ensure_slots(1);
            rust_wren::value::ToWren::put(result, &mut ctx, 0);
        }
    };

    Ok(quote! {
        #method

        #wrap
    })
}

/// Generate arguments to a function call that extracts values from Wren slots.
///
/// # Arguments
///
/// - `ctx` - Identifier of the `WrenContext` that will be in scope for the method call.
///
/// # Receivers
///
/// Receivers come in various flavours, and need to be borrowed from the
/// `WrenCell` wrapping the type.
///
/// - `self` - We only support cloning, and not moving, the value out of Wren.
/// - `&self` - Value is borrowed from the `WrenCell`.
/// - `&mut self` - Value is borrowed mutably from the `WrenCell`.
///
/// Currently receivers of type `Box`, `Rc`, `Arc` and `Pin`
/// are not supported.
fn gen_args_from_slots(ctx: &Ident, method: &ImplItemMethod, is_static: bool) -> syn::Result<Vec<TokenStream>> {
    let method_name = method.sig.ident.to_string();

    // When Wren calls a static method, the first slot will
    // have the class as a receiver. It is of type UNKNOWN
    // and not really usable in a Rust function for anything.
    //
    // Since there is no argument in the Rust function
    // corresponding to the static receiver, we need to
    // offset the slot position by 1.
    //
    // An instance method would have slot 0 correspond to the
    // Rust `self` receiver.
    let offset = if is_static { 1 } else { 0 };

    let args = method.sig.inputs
        .iter()
        // Argument positions correlate to Wren slot positions.
        .enumerate()
        .map(|(idx, arg)| {
            let idx_lit = Lit::new(Literal::i32_unsuffixed(idx as i32 + offset));

            match arg {
                FnArg::Receiver(recv) => {
                    // TODO: When receiver's `reference` field is None, it's
                    //       a move. The `WrenCell` should be borrowed and cloned.
                    //       The user would need an error with a nice span
                    //       indicating the type needs to implement clone.
                    //       Could be useful for value types like Vectors or Matrices.
                    //       We can't really move out of Wren, because we can't take
                    //       ownership of the memory until it's garbage collected.
                    // let is_ref = rect.reference.is_some();

                    // Important semantic choice that allows the user
                    // to pass a foreign class as receiver and function
                    // arguments as long as everything is immutable.
                    let is_mut = recv.mutability.is_some();
                    let borrow_call = if is_mut {
                        format_ident!("try_borrow_mut")
                    } else {
                        format_ident!("try_borrow")
                    };

                    // Borrow the inner value of the returned `RefMut<Self>` or `Ref<Self>`;
                    let borrow_return = if is_mut {
                        quote! { &mut *result.unwrap() }
                    } else {
                        quote! { &*result.unwrap() }
                    };

                    quote! {
                        {
                            // let ref_cell: &mut ::rust_wren::class::WrenCell<Self> = #ctx.get_slot::<Self>(#idx_lit)
                            //     .unwrap_or_else(|err| panic!("Getting slot {} for method '{}' failed: {}", #idx_lit, #method_name, err));
                            let result = #ctx.get_slot::<Self>(#idx_lit)
                                .and_then(|wren_cell| wren_cell.#borrow_call())
                                .map_err(|err| {
                                    let wren_error = rust_wren::WrenError::new_foreign_call(
                                            #method_name,
                                            Box::new(rust_wren::WrenError::GetArg { slot: #idx_lit, cause: err.into(), })
                                        );

                                    rust_wren::ForeignError::Simple(Box::new(wren_error))
                                });

                            // Scoping around `RefCell`, `Ref` and `RefMut` are tricky.
                            //
                            // If the `Ref` or `RefMut` end up in variables in this block,
                            // then they will be dropped while
                            if let Err(foreign_error) = result {
                                // `ForeignError` is our mechanism for aborting a Wren fiber
                                // with a Rust error.
                                foreign_error.put(&mut ctx, 0);
                                return;
                            }

                            #borrow_return
                        }
                    }
                }
                FnArg::Typed(pat_ty) => {
                    let arg_type = pat_ty.ty.clone();
                    quote! {
                        match ctx.get_slot::<#arg_type>(#idx_lit) {
                            Ok(value) => value,
                            Err(err) => {
                                let wren_error = rust_wren::WrenError::new_foreign_call(
                                        #method_name,
                                        Box::new(rust_wren::WrenError::GetArg { slot: #idx_lit, cause: err.into(), })
                                    );

                                // `ForeignError` is our mechanism for aborting a Wren fiber
                                // with a Rust error.
                                let foreign_error = rust_wren::ForeignError::Simple(Box::new(wren_error));
                                foreign_error.put(&mut ctx, 0);

                                return;
                            }
                        }
                    }
                }
            }
        })
        .collect();

    Ok(args)
}

fn gen_register(wrappers: &[WrenFnSpec]) -> syn::Result<TokenStream> {
    let calls = wrappers
        .iter()
        .map(|spec| {
            let is_static = Ident::new(spec.is_static.to_string().as_str(), Span::call_site());
            let arity = Literal::usize_unsuffixed(spec.arity);
            let sig = Literal::string(spec.sig.as_str());

            let wrap_ident = spec.wrap_ident.clone();
            let func = quote! { #wrap_ident };

            quote! {
                builder.add_method_binding(
                    <Self as rust_wren::class::WrenForeignClass>::NAME,
                    rust_wren::foreign::ForeignMethod {
                        is_static: #is_static,
                        arity: #arity,
                        sig: #sig.to_owned(),
                        func: <Self>::#func,
                })
            }
        })
        .collect::<Vec<_>>();

    Ok(quote! {
        extern "C" fn __wren_register_methods(builder: &mut rust_wren::ModuleBuilder) {
            #(#calls);*
        }
    })
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct WrenFnSpec {
    /// Identifier for method.
    ident: Ident,
    /// Identifier for the C function wrapping this method.
    wrap_ident: Ident,
    /// Arguments passed to the optional method attribute.
    args: WrenMethodArgs,
    /// Spec type that distinguishes generation behaviour.
    ty: WrenFnType,
    /// Number of function parameters, excluding self.
    arity: usize,
    /// Wren function signature as string.
    sig: String,
    /// Indicates whether the method is static and does
    /// not accept an instance as a receiver.
    is_static: bool,
    /// Indicates whether the method is the class constructor.
    is_construct: bool,
}

impl WrenFnSpec {
    pub fn build(sig: &Signature, attrs: &mut Vec<Attribute>) -> syn::Result<Self> {
        let ident = sig.ident.clone();
        let wrap_ident = format_ident!("__wren_wrap_{}", ident);
        let args = WrenMethodArgs::build_args(attrs)?;

        // Note that self receivers with a specified type, such as self: Box<Self>, are parsed as a FnArg::Typed.
        // https://docs.rs/syn/1.0.48/syn/enum.FnArg.html
        let is_static = sig.inputs.iter().all(|arg| !matches!(arg, FnArg::Receiver(_))) || sig.inputs.is_empty();

        // Wren does not include the receiver in the function signature, but Rust does.
        let arity = if !sig.inputs.is_empty() {
            if is_static {
                sig.inputs.len()
            } else {
                sig.inputs.len() - 1
            }
        } else {
            0
        };

        let wren_sig = Self::make_wren_signature(sig, args.name.as_ref());

        if attrs.iter().any(|attr| attr.path.is_ident("construct")) {
            // Constructor
            if is_static {
                Ok(WrenFnSpec {
                    ident,
                    wrap_ident,
                    args,
                    ty: WrenFnType::Construct,
                    arity,
                    sig: wren_sig,
                    is_static,
                    is_construct: true,
                })
            } else {
                Err(syn::Error::new_spanned(
                    sig,
                    "Construct method must be static, ie. not receive `self`",
                ))
            }
        } else {
            Ok(WrenFnSpec {
                ident,
                wrap_ident,
                args,
                ty: WrenFnType::Method,
                arity,
                sig: wren_sig,
                is_static,
                is_construct: false,
            })
        }
    }

    /// Create a Wren call signature.
    fn make_wren_signature(sig: &Signature, wren_name: Option<&Ident>) -> String {
        // Wren name can be specified using a attribute, else use Rust identifier.
        let mut sb = wren_name.unwrap_or_else(|| &sig.ident).to_string();
        // Note that self receivers with a specified type, such as self: Box<Self>, are parsed as a FnArg::Typed.
        // https://docs.rs/syn/1.0.48/syn/enum.FnArg.html
        let args = sig
            .inputs
            .iter()
            .filter(|arg| !matches!(arg, FnArg::Receiver(_)))
            .map(|_| "_")
            .collect::<Vec<&'static str>>()
            .join(",");
        sb.push_str("(");
        sb.push_str(&args);
        sb.push_str(")");
        sb
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum WrenFnType {
    Construct,
    Method,
    Operator,
}

/// Arguments to method attribute on an associated function.
#[derive(Debug, Default)]
struct WrenMethodArgs {
    name: Option<Ident>,
}

impl Parse for WrenMethodArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = WrenMethodArgs::default();

        let content;
        parenthesized!(content in input);

        let vars = Punctuated::<Expr, Token![,]>::parse_terminated(&content)?;
        for expr in vars {
            args.add_expr(&expr)?;
        }

        Ok(args)
    }
}

impl WrenMethodArgs {
    fn build_args(attrs: &mut Vec<Attribute>) -> syn::Result<Self> {
        if attrs.is_empty() {
            return Ok(WrenMethodArgs::default());
        }

        // Find pertinent attribute.
        let attr_ident = format_ident!("method");
        let maybe_attr_pos = attrs
            .iter()
            .filter(|attr| attr.path.get_ident().is_some())
            .position(|attr| attr.path.get_ident() == Some(&attr_ident));

        if let Some(index) = maybe_attr_pos {
            // Keeping the attribute would cause a compile error
            // since the compiler doesn't know what to do with it.
            let tokens = attrs.remove(index).tokens.clone();
            let args = syn::parse2(tokens)?;

            Ok(args)
        } else {
            Ok(WrenMethodArgs::default())
        }
    }

    fn add_expr(&mut self, expr: &Expr) -> syn::parse::Result<()> {
        match expr {
            Expr::Assign(assign) => self.add_assign(assign),
            _ => Err(syn::parse::Error::new_spanned(expr, "Failed to parse arguments")),
        }
    }

    fn add_assign(&mut self, expr: &ExprAssign) -> syn::parse::Result<()> {
        let ExprAssign { left, right, .. } = expr;

        let key = match &**left {
            Expr::Path(path_expr) if path_expr.path.segments.len() == 1 => {
                path_expr.path.segments.first().unwrap().ident.to_string()
            }
            _ => return Err(syn::Error::new_spanned(expr, "Failed to parse arguments")),
        };

        match key.as_str() {
            "name" => match &**right {
                Expr::Path(right_expr) if right_expr.path.segments.len() == 1 => {
                    self.name = right_expr.path.get_ident().cloned();
                }
                _ => return Err(syn::parse::Error::new_spanned(expr, "Expected class name")),
            },
            _ => return Err(syn::Error::new_spanned(expr, "Failed to parse arguments")),
        }

        Ok(())
    }
}
