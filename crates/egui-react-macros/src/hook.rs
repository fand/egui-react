//! `#[hook]`: give a custom hook its own id scope.

use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;

use crate::util::{arg_ident, is_cx_ref, to_compile_error};

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    match try_expand(attr, item.clone()) {
        Ok(tokens) => tokens,
        // Keep the original item so that the rest of the file still resolves
        // and the IDE keeps working.
        Err(err) => {
            let err = to_compile_error(err);
            quote! { #item #err }
        }
    }
}

fn try_expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "egui-react: #[hook] takes no arguments",
        ));
    }
    let mut function: syn::ItemFn = syn::parse2(item)?;

    let cx = function
        .sig
        .inputs
        .iter()
        .find_map(|arg| match arg {
            syn::FnArg::Typed(arg) if is_cx_ref(&arg.ty) => Some(arg_ident(arg)),
            _ => None,
        })
        .transpose()?
        .ok_or_else(|| {
            syn::Error::new(
                function.sig.span(),
                "egui-react: a #[hook] function must take a `&mut Cx` argument; \
                 that is the context whose scope the hook is entered under",
            )
        })?;

    let body = &function.block;
    // `hook_scope` shadows `cx` inside the closure, so the body is unchanged.
    function.block = syn::parse_quote!({
        let __egui_react_location = ::std::panic::Location::caller();
        #cx.hook_scope(__egui_react_location, |#cx| #body)
    });
    function.attrs.push(syn::parse_quote!(#[track_caller]));

    Ok(quote! { #function })
}
