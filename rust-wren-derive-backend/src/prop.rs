//! Class property generation.
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{spanned::Spanned, Fields, ItemStruct, Type};

pub fn gen_class_props(class: &ItemStruct) -> syn::Result<TokenStream> {
    let get_set = format_ident!("getset");
    let get = format_ident!("get");
    let set = format_ident!("set");

    let mut registers = vec![];
    let mut gets = vec![];
    let mut sets = vec![];
    let mut assert_clone = vec![];

    for (idx, field) in class.fields.iter().enumerate() {
        for attr in &field.attrs {
            // Tuple struct fields don't have identifiers, so we
            // have to access it via an integer identifier.
            let field_ident = match field.ident {
                Some(ref ident) => ident.clone(),
                None => format_ident!("{}", idx),
            };

            let field_ty = field.ty.clone();

            // Compile time assertion to provide user friendly error
            // when property does not implement `Clone`.
            let field_span = field_ty.span();
            let assert_ident = format_ident!("_{}_AssertSync", field_ident);
            assert_clone.push(quote_spanned! {field_span=>
                #[allow(non_camel_case_types)]
                struct #assert_ident where #field_ty: Clone;
            });

            match attr.path.get_ident() {
                ident if ident == Some(&get) => {
                    let (g, r) = gen_get(&field_ident);
                    gets.push(g);
                    registers.push(r);
                }
                ident if ident == Some(&set) => {
                    let (s, r) = gen_set(&field_ident, &field_ty);
                    sets.push(s);
                    registers.push(r);
                }
                ident if ident == Some(&get_set) => {
                    let (g, r) = gen_get(&field_ident);
                    gets.push(g);
                    registers.push(r);

                    let (s, r) = gen_set(&field_ident, &field_ty);
                    sets.push(s);
                    registers.push(r);
                }
                _ => {}
            }
        }
    }

    let ty = class.ident.clone();

    let gen = quote! {
        #(#assert_clone)*

        #[doc(hidden)]
        impl #ty {
            #(#gets)*
            #(#sets)*

            fn __wren_register_properties(builder: &mut rust_wren::ModuleBuilder) {
                #(#registers)*
            }
        }
    };

    Ok(gen)
}

/// Generate property get function.
fn gen_get(field_ident: &Ident) -> (TokenStream, TokenStream) {
    // Signature of a property get is simply the property name; no parentheses or argument arity.
    let sig = field_ident.to_string();
    let wrap_func = format_ident!("__wren_wrap_get_{}", field_ident);
    let span = field_ident.span();

    let get = quote_spanned! {span=>
        extern "C" fn #wrap_func(vm: *mut rust_wren::bindings::WrenVM) {
            // Context for extracting slots.
            let vm: &mut rust_wren::bindings::WrenVM = unsafe { vm.as_mut().unwrap() };
            let mut ctx = rust_wren::WrenContext::new(vm);

            // Retrieve receiver, which contains the property value.
            let cell = match ctx.get_slot::<Self>(0) {
                Ok(cell) => cell,
                Err(err) => {
                    let wren_error = rust_wren::WrenError::new_foreign_call(
                        #sig,
                        Box::new(rust_wren::WrenError::GetArg { slot: 0, cause: err.into(), })
                    );

                    let foreign_error = rust_wren::ForeignError::Simple(Box::new(wren_error));
                    foreign_error.put(&mut ctx, 0);

                    return;
                }
            };

            // Value must be cloned to be sent from Rust to Wren.
            let prop = match cell.try_borrow_mut() {
                Ok(ref mut self_) => self_.#field_ident.clone(),
                Err(err) => {
                    let wren_error = rust_wren::WrenError::new_foreign_call(
                        #sig,
                        Box::new(rust_wren::WrenError::GetArg { slot: 0, cause: err.into(), })
                    );

                    let foreign_error = rust_wren::ForeignError::Simple(Box::new(wren_error));
                    foreign_error.put(&mut ctx, 0);

                    return;
                }
            };

            // Property return value goes into the first slot.
            rust_wren::value::ToWren::put(prop, &mut ctx, 0);
        }
    };

    let register = quote! {
        builder.add_method_binding(
            <Self as rust_wren::class::WrenForeignClass>::NAME,
            rust_wren::foreign::ForeignMethod {
                is_static: false,
                arity: 0,
                sig: #sig.to_owned(),
                func: <Self>::#wrap_func,
            }
        );
    };

    (get, register)
}

/// Generate property set function.
fn gen_set(field_ident: &Ident, field_ty: &Type) -> (TokenStream, TokenStream) {
    // Signature of a property assign is the property name followed by an equal sign.
    let sig = format!("{}=(_)", field_ident);
    let wrap_func = format_ident!("__wren_wrap_set_{}", field_ident);
    let span = field_ident.span();

    let set = quote_spanned! {span=>
        extern "C" fn #wrap_func(vm: *mut rust_wren::bindings::WrenVM) {
            // Context for extracting slots.
            let vm: &mut rust_wren::bindings::WrenVM = unsafe { vm.as_mut().unwrap() };
            let mut ctx = rust_wren::WrenContext::new(vm);

            // Retrieve receiver, which is where we'll be storing the new property value.
            // let cell = ctx.get_slot::<Self>(0)
            //     .unwrap_or_else(|err| panic!("Getting receiver from slot 0 for property '{}' failed: {}", #sig, err));
            let cell = match ctx.get_slot::<Self>(0) {
                Ok(cell) => cell,
                Err(err) => {
                    // TODO: Wrap this up in a macro.
                    let wren_error = rust_wren::WrenError::new_foreign_call(
                        #sig,
                        Box::new(rust_wren::WrenError::GetArg { slot: 0, cause: err.into(), })
                    );

                    let foreign_error = rust_wren::ForeignError::Simple(Box::new(wren_error));
                    foreign_error.put(&mut ctx, 0);

                    return;
                }
            };

            // Setters always have only one argument.
            // ctx.get_slot::<#field_ty>(1).unwrap_or_else(|err| panic!("Getting value from slot 1 for property '{}' failed: {}", #sig, err));
            let value = match ctx.get_slot::<#field_ty>(1) {
                Ok(value) => value,
                Err(err) => {
                    let wren_error = rust_wren::WrenError::new_foreign_call(
                        #sig,
                        Box::new(rust_wren::WrenError::GetArg { slot: 0, cause: err.into(), })
                    );

                    let foreign_error = rust_wren::ForeignError::Simple(Box::new(wren_error));
                    foreign_error.put(&mut ctx, 0);

                    return;
                }
            };

            // Property value must be cloneable because it is assigned to the Rust struct
            // and also returned later.
            // cell.borrow_mut().#field_ident = value.clone();
            match cell.try_borrow_mut() {
                Ok(ref mut self_) => self_.#field_ident = value.clone(),
                Err(err) => {
                    let wren_error = rust_wren::WrenError::new_foreign_call(
                        #sig,
                        Box::new(rust_wren::WrenError::GetArg { slot: 0, cause: err.into(), })
                    );

                    let foreign_error = rust_wren::ForeignError::Simple(Box::new(wren_error));
                    foreign_error.put(&mut ctx, 0);

                    return;
                }
            }

            // To keep with the convention of assignment returning the
            // assigned value, we copy the value to the return slot.
            rust_wren::value::ToWren::put(value, &mut ctx, 0);
        }
    };

    let register = quote! {
        builder.add_method_binding(
            <Self as rust_wren::class::WrenForeignClass>::NAME,
            rust_wren::foreign::ForeignMethod {
                is_static: false,
                arity: 0,
                sig: #sig.to_owned(),
                func: <Self>::#wrap_func,
            }
        );
    };

    (set, register)
}

/// Remove known attributes, otherwise compilation would fail after code gen.
pub fn strip_prop_attrs(fields: &mut Fields) {
    let getset_ident = format_ident!("getset");
    let get_ident = format_ident!("get");
    let set_ident = format_ident!("set");
    let all = [getset_ident, get_ident, set_ident];

    for field in fields {
        let maybe_attr_pos = field
            .attrs
            .iter()
            .filter(|attr| attr.path.get_ident().is_some())
            .position(|attr| {
                if let Some(attr_ident) = attr.path.get_ident() {
                    if all.contains(attr_ident) {
                        return true;
                    }
                }

                false
            });

        if let Some(index) = maybe_attr_pos {
            // Keeping the attribute would cause a compile error
            // since the compiler doesn't know what to do with it.
            field.attrs.remove(index);
        }
    }
}
