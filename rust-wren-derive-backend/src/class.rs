//! `wren_class` attribute.
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Expr, ExprAssign, Ident, Token,
};

/// Generate an implementation of `FromWren` for the given type identifier.
///
/// The type must be `'static` since ownership will be passed to Wren.
pub fn gen_from_wren_impl(class: Ident) -> TokenStream {
    quote! {
        /// Currently this implementation is only really used for
        /// function receivers. Function arguments need `WrenCell`.
        impl<'wren> FromWren<'wren> for #class {
            type Output = &'wren mut rust_wren::class::WrenCell<Self>;

            fn get_slot(ctx: &rust_wren::WrenContext, slot_num: i32) -> rust_wren::WrenResult<Self::Output> {
                <&mut WrenCell<Self>>::get_slot(ctx, slot_num)
            }
        }
    }
}

/// Generate an implementation of `ToWren` for the given type identifier.
pub fn gen_to_wren_impl(class: Ident) -> TokenStream {
    quote! {
        /// Moves the given [`WrenForeignClass`] instance, with `'static` type, into Wren.
        ///
        /// Once a value is moved into Wren, it cannot be moved out. Only a
        /// mutable reference can be retrieved as a receiver to one of its Rust
        /// methods.
        ///
        /// The foreign class binding must be registered for the type to be
        /// moved into Wren.
        ///
        /// This implementation is effectively the same as the generated `__wren_allocate` method.
        ///
        /// # Safety
        ///
        /// Ensure slots must be called first to ensure Wren has a slot for this value.
        ///
        /// # Errors
        ///
        /// Returns an error if the foreign class binding cannot be found.
        ///
        /// # Implementation
        ///
        /// Allocates space in Wren's heap to contain the value.
        impl rust_wren::value::ToWren for #class {
            fn put(self, ctx: &mut rust_wren::WrenContext, slot: i32) {
                use rust_wren::{prelude::*, bindings, value::ToWren};
                assert!((slot as usize) < ctx.slot_count());

                // To allocate a new foreign object, we must first lookup its class.
                let module_name = {
                    let userdata = unsafe { ctx.user_data().unwrap() }; // TODO: Return Error
                    userdata
                        .foreign
                        .get_class_key::<Self>()
                        .unwrap()
                        .module
                        .clone()
                };
                let class_name = <Self as WrenForeignClass>::NAME;

                // Class declarations are simple variables in Wren.
                let class_ref = ctx.get_var(&module_name, class_name).unwrap();

                // Prepare for foreign value allocation.
                ToWren::put(class_ref, ctx, slot as i32);

                // Wren wants to own the memory containing the data backing the foreign function.
                let wren_ptr: *mut WrenCell<Self> = unsafe {
                    bindings::wrenSetSlotNewForeign(
                        ctx.vm_ptr(),
                        slot as i32,
                        slot as i32,
                        ::std::mem::size_of::<WrenCell<Self>>() as usize,
                    ) as _
                };
                let wren_val: &mut WrenCell<Self> = unsafe { wren_ptr.as_mut().unwrap() };

                // All foreign classes are wrapped in WrenCell, because it's possible to
                // borrow the value out of Wren multiple times.
                let mut rust_val = WrenCell::new(self);

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
        }
    }
}

/// Arguments used for annotating a struct as a Wren class.
#[derive(Default)]
pub struct WrenClassArgs {
    pub name: Option<syn::Expr>,
}

impl Parse for WrenClassArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = WrenClassArgs::default();

        let vars = Punctuated::<Expr, Token![,]>::parse_terminated(input)?;
        for expr in vars {
            args.add_expr(&expr)?;
        }

        Ok(args)
    }
}

impl WrenClassArgs {
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
                    self.name = Some(right_expr.clone().into());
                }
                _ => return Err(syn::parse::Error::new_spanned(expr, "Expected class name")),
            },
            "base" => {}
            _ => return Err(syn::Error::new_spanned(expr, "Failed to parse arguments")),
        }

        Ok(())
    }
}

/// TODO: Inventory to register methods on binary execute.
#[allow(dead_code)]
fn gen_class_invetory(cls_ident: &Ident) -> syn::Result<TokenStream> {
    let inv_cls = format_ident!("WrenClassInvestory__{}", cls_ident);

    Ok(quote! {
        struct #inv_cls {
            methods: Vec<::rust_wren::ForeignMethods>,
        }

        impl #inv_cls {
            fn new() -> Self {
                Self { methods: vec![] }
            }
        }


    })
}
