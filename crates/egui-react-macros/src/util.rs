//! Small helpers shared by the three macros.

use proc_macro2::TokenStream;
use quote::quote_spanned;
use syn::spanned::Spanned;

/// Turn `syn::Error` into a `compile_error!` invocation.
pub(crate) fn to_compile_error(err: syn::Error) -> TokenStream {
    err.to_compile_error()
}

/// Emit `compile_error!(msg)` at `span` and nothing else.
pub(crate) fn error_at(span: proc_macro2::Span, msg: &str) -> TokenStream {
    quote_spanned! { span => ::core::compile_error!(#msg); }
}

/// Whether `ty` is `&mut Cx<..>` (or `&mut Cx`), the first argument every
/// component and hook takes.
pub(crate) fn is_cx_ref(ty: &syn::Type) -> bool {
    let syn::Type::Reference(reference) = ty else {
        return false;
    };
    if reference.mutability.is_none() {
        return false;
    }
    let syn::Type::Path(path) = &*reference.elem else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Cx")
}

/// The identifier a `fn` argument binds, if it is a plain `name: T`.
pub(crate) fn arg_ident(arg: &syn::PatType) -> syn::Result<syn::Ident> {
    match &*arg.pat {
        syn::Pat::Ident(pat) => Ok(pat.ident.clone()),
        other => Err(syn::Error::new(
            other.span(),
            "egui-react: this argument must be a plain `name: Type` binding",
        )),
    }
}

/// Whether `ty` is spelled `Option<..>`.
pub(crate) fn is_option(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none()
        && path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Option")
}

/// `on_value_change` -> `ValueChange`.
pub(crate) fn event_variant_name(attr_name: &str) -> String {
    let rest = attr_name.strip_prefix("on_").unwrap_or(attr_name);
    let mut out = String::new();
    for word in rest.split('_') {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

/// Strip the braces off `{ expr }`.
///
/// `rsx!` attribute values and `{expr}` nodes arrive as blocks; passing the
/// block straight through would make `unused_braces` fire on user code.
pub(crate) fn unwrap_braced(expr: &syn::Expr) -> TokenStream {
    if let syn::Expr::Block(block) = expr
        && block.attrs.is_empty()
        && block.label.is_none()
        && let [syn::Stmt::Expr(inner, None)] = block.block.stmts.as_slice()
    {
        return quote::quote!(#inner);
    }
    quote::quote!(#expr)
}
