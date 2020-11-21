//! `wren_class` attribute.
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Expr, ExprAssign, Ident, Token,
};

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
            _ => Err(syn::parse::Error::new_spanned(
                expr,
                "Failed to parse arguments",
            )),
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

/// TODO
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
