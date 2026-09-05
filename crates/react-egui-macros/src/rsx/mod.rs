//! `rsx!`: parse JSX-like syntax and expand it into direct `Cx` calls.

mod control_flow;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use rstml::node::{KeyedAttributeValue, Node, NodeAttribute, NodeElement, NodeName};
use rstml::{Parser, ParserConfig};
use syn::spanned::Spanned;

use crate::util::{error_at, event_variant_name, to_compile_error, unwrap_braced};
use control_flow::{ControlFlow, ElseBranch, IfNode};

/// Layout attributes every element accepts, forwarded to `ItemStyle`.
const LAYOUT_ATTRS: &[&str] = &[
    "w",
    "h",
    "min_w",
    "min_h",
    "max_w",
    "max_h",
    "grow",
    "shrink",
    "basis",
    "align_self",
    "m",
    "mx",
    "my",
    "mt",
    "mr",
    "mb",
    "ml",
    "p",
    "px",
    "py",
    "pt",
    "pr",
    "pb",
    "pl",
    "col_span",
    "row_span",
];

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    let config = ParserConfig::new().custom_node::<ControlFlow>();
    let (nodes, diagnostics) = Parser::new(config).parse_recoverable(input).split_vec();
    let errors = diagnostics.into_iter().map(|d| d.emit_as_expr_tokens());

    let mut expander = Expander::default();
    let body = expander.nodes(&nodes);
    // A malformed node (bare text, a doctype) makes the rest of the expansion
    // meaningless, so report only that instead of a cascade of type errors.
    let hard_errors = &expander.errors;
    if !hard_errors.is_empty() {
        return quote! {
            {
                #(#errors)*
                #(#hard_errors)*
                ::react_egui::view(|_cx| {})
            }
        };
    }
    quote! {
        {
            #(#errors)*
            ::react_egui::view(|cx| { #body })
        }
    }
}

/// Walks the node tree, numbering elements so each gets a distinct scope id.
#[derive(Default)]
struct Expander {
    element_index: usize,
    /// Errors that make the whole expansion pointless.
    errors: Vec<TokenStream>,
}

impl Expander {
    /// Expand a list of nodes into a sequence of statements.
    fn nodes(&mut self, nodes: &[Node<ControlFlow>]) -> TokenStream {
        let stmts = nodes.iter().map(|node| self.node(node));
        quote! { #(#stmts)* }
    }

    fn node(&mut self, node: &Node<ControlFlow>) -> TokenStream {
        match node {
            Node::Element(element) => self.element(element),
            Node::Text(text) => {
                let value = &text.value;
                quote! { ::react_egui::View::show(#value, cx); }
            }
            Node::Block(block) => {
                let expr = block_expr(block);
                quote! { ::react_egui::View::show(#expr, cx); }
            }
            Node::Fragment(fragment) => self.nodes(&fragment.children),
            Node::Comment(_) => quote! {},
            Node::RawText(raw) => {
                self.errors.push(error_at(
                    raw.span(),
                    "react-egui: text must be a string literal: \"...\"",
                ));
                quote! {}
            }
            Node::Doctype(doctype) => {
                self.errors.push(error_at(
                    doctype.span(),
                    "react-egui: <!DOCTYPE> has no meaning in rsx!",
                ));
                quote! {}
            }
            Node::Custom(control_flow) => self.control_flow(control_flow),
        }
    }

    fn control_flow(&mut self, node: &ControlFlow) -> TokenStream {
        match node {
            ControlFlow::If(node) => self.if_node(node),
            ControlFlow::For(node) => {
                let (pat, expr) = (&node.pat, &node.expr);
                let body = self.nodes(&node.body);
                quote! { for #pat in #expr { #body } }
            }
            ControlFlow::Match(node) => {
                let expr = &node.expr;
                let arms: Vec<TokenStream> = node
                    .arms
                    .iter()
                    .map(|arm| {
                        let pat = &arm.pat;
                        let guard = arm.guard.as_ref().map(|guard| quote!(if #guard));
                        let body = self.nodes(&arm.body);
                        quote! { #pat #guard => { #body } }
                    })
                    .collect();
                quote! { match #expr { #(#arms)* } }
            }
        }
    }

    fn if_node(&mut self, node: &IfNode) -> TokenStream {
        let condition = &node.condition;
        let then_branch = self.nodes(&node.then_branch);
        let else_branch = match node.else_branch.as_deref() {
            Some(ElseBranch::If(nested)) => {
                let nested = self.if_node(nested);
                Some(quote!(else #nested))
            }
            Some(ElseBranch::Block(nodes)) => {
                let nodes = self.nodes(nodes);
                Some(quote!(else { #nodes }))
            }
            None => None,
        };
        quote! { if #condition { #then_branch } #else_branch }
    }

    /// `<Name attr=..>children</Name>` becomes one scoped component call.
    fn element(&mut self, element: &NodeElement<ControlFlow>) -> TokenStream {
        let index = self.element_index;
        self.element_index += 1;

        let path = match element.name() {
            NodeName::Path(path) => path.clone(),
            other => {
                return error_at(
                    other.span(),
                    "react-egui: an element name must be a component path, like <Button/> or \
                     <elements::Button/>",
                );
            }
        };
        let span = path.span();

        let attrs = match Attributes::parse(element, &path) {
            Ok(attrs) => attrs,
            Err(err) => return to_compile_error(err),
        };

        let setters = attrs.setters.iter().map(|(name, value)| {
            quote_spanned! { name.span() => .#name(#value) }
        });
        let style = attrs.style_call();
        let events = attrs.events_call();
        let children = self.children(&element.children);

        let key = attrs.key.as_ref().map(|key| quote!(, #key));
        let source = quote_spanned! { span =>
            (::core::file!(), ::core::line!(), ::core::column!(), #index #key)
        };

        quote_spanned! { span =>
            cx.scope(#source, |cx| {
                #path(
                    cx,
                    ::react_egui::props_builder(&#path)
                        #(#setters)*
                        #style
                        #events
                        .children(#children)
                        .build(),
                );
            });
        }
    }

    /// The `children` argument for one element.
    ///
    /// A lone string literal or a lone `{expr}` is passed straight through, so
    /// that `<Button>"OK"</Button>` can feed an `impl Into<WidgetText>` prop.
    fn children(&mut self, children: &[Node<ControlFlow>]) -> TokenStream {
        match children {
            [] => quote!(()),
            [Node::Text(text)] => {
                let value = &text.value;
                quote!(#value)
            }
            [Node::Block(block)] => block_expr(block),
            nodes => {
                let body = self.nodes(nodes);
                quote! { ::react_egui::view(|cx| { #body }) }
            }
        }
    }
}

/// The attributes of one element, sorted into their four kinds.
#[derive(Default)]
struct Attributes {
    key: Option<syn::Expr>,
    /// `(setter name, value)` for ordinary props.
    setters: Vec<(syn::Ident, TokenStream)>,
    /// `(ItemStyle setter name, value)`.
    layout: Vec<(syn::Ident, TokenStream)>,
    /// `(event enum path, variant, handler)`.
    handlers: Vec<(syn::Path, syn::Ident, TokenStream)>,
    /// `events={..}`, the escape hatch.
    events: Option<TokenStream>,
}

impl Attributes {
    fn parse(element: &NodeElement<ControlFlow>, path: &syn::ExprPath) -> syn::Result<Self> {
        let event_enum = event_enum_path(path);
        let mut out = Attributes::default();
        let mut seen: Vec<String> = Vec::new();

        for attr in element.attributes() {
            let NodeAttribute::Attribute(attr) = attr else {
                return Err(syn::Error::new(
                    attr.span(),
                    "react-egui: block attributes (`<Name {expr}/>`) are not supported",
                ));
            };
            let NodeName::Path(key_path) = &attr.key else {
                return Err(syn::Error::new(
                    attr.key.span(),
                    "react-egui: an attribute name must be a plain identifier",
                ));
            };
            let Some(name) = key_path.path.get_ident().cloned() else {
                return Err(syn::Error::new(
                    attr.key.span(),
                    "react-egui: an attribute name must be a plain identifier",
                ));
            };
            let text = name.to_string();
            if seen.contains(&text) {
                return Err(syn::Error::new(
                    name.span(),
                    format!("react-egui: duplicate attribute `{text}`"),
                ));
            }
            seen.push(text.clone());

            // `name` on its own is the boolean `true`.
            let value = match &attr.possible_value {
                KeyedAttributeValue::None => quote_spanned!(name.span() => true),
                KeyedAttributeValue::Value(value) => match &value.value {
                    rstml::node::KVAttributeValue::Expr(expr) => unwrap_braced(expr),
                    invalid => {
                        return Err(syn::Error::new(
                            invalid.span(),
                            "react-egui: this attribute value is not a valid expression",
                        ));
                    }
                },
                KeyedAttributeValue::Binding(binding) => {
                    return Err(syn::Error::new(
                        binding.paren.span.span(),
                        "react-egui: closure-binding attributes are not supported",
                    ));
                }
            };

            if text == "key" {
                out.key = Some(syn::parse2(value)?);
            } else if text == "events" {
                out.events = Some(value);
            } else if let Some(variant) = text.strip_prefix("on_") {
                let variant = syn::Ident::new(&event_variant_name(variant), name.span());
                out.handlers.push((event_enum.clone(), variant, value));
            } else if LAYOUT_ATTRS.contains(&text.as_str()) {
                out.layout.push((name, value));
            } else {
                out.setters.push((name, value));
            }
        }

        if out.events.is_some() && !out.handlers.is_empty() {
            return Err(syn::Error::new(
                element.name().span(),
                "react-egui: `events=` replaces the fused `on_*` closure; use one or the other",
            ));
        }
        Ok(out)
    }

    /// `.style(ItemStyle::default().w(..)..)`, or nothing.
    fn style_call(&self) -> Option<TokenStream> {
        if self.layout.is_empty() {
            return None;
        }
        let setters = self.layout.iter().map(|(name, value)| {
            quote_spanned! { name.span() => .#name(#value) }
        });
        Some(quote! {
            .style(::react_egui::layout::ItemStyle::default() #(#setters)*)
        })
    }

    /// `.events(&mut |ev| match ev { .. })`, or the escape hatch, or nothing.
    fn events_call(&self) -> Option<TokenStream> {
        if let Some(events) = &self.events {
            return Some(quote! { .events(&mut (#events)) });
        }
        if self.handlers.is_empty() {
            return None;
        }
        let arms = self.handlers.iter().map(|(enum_path, variant, handler)| {
            quote_spanned! { variant.span() =>
                #enum_path::#variant(__react_egui_payload) => {
                    ::react_egui::Handler::call(#handler, __react_egui_payload)
                }
            }
        });
        Some(quote! {
            .events(&mut |__react_egui_event| {
                #[allow(unreachable_patterns)]
                match __react_egui_event {
                    #(#arms)*
                    _ => {}
                }
            })
        })
    }
}

/// The expression inside `{ .. }`, unwrapped when the block is a single
/// expression so that `unused_braces` does not fire in user code.
fn block_expr(block: &rstml::node::NodeBlock) -> TokenStream {
    if let rstml::node::NodeBlock::ValidBlock(block) = block
        && let [syn::Stmt::Expr(expr, None)] = block.stmts.as_slice()
    {
        return quote!(#expr);
    }
    quote!(#block)
}

/// `elements::Button` -> `elements::ButtonEvent`.
fn event_enum_path(path: &syn::ExprPath) -> syn::Path {
    let mut path = path.path.clone();
    if let Some(last) = path.segments.last_mut() {
        last.ident = format_ident!("{}Event", last.ident, span = last.ident.span());
        last.arguments = syn::PathArguments::None;
    }
    path
}
