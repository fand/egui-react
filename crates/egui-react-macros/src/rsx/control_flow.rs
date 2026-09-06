//! The `rstml` custom node: `if` / `for` / `match` written directly in `rsx!`.
//!
//! Direct expansion means these can be the real Rust statements, so a `for`
//! body can borrow the loop variable and an `if` can borrow a `State` guard.

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use rstml::node::{CustomNode, Node};
use rstml::recoverable::{ParseRecoverable, RecoverableContext};
use syn::parse::ParseStream;
use syn::{Token, braced, token};

/// A `rsx!` node that is a Rust control-flow construct.
///
/// The variants are boxed because a `syn::Expr` alone is several hundred bytes.
#[derive(Debug)]
pub(crate) enum ControlFlow {
    If(Box<IfNode>),
    For(Box<ForNode>),
    Match(Box<MatchNode>),
}

/// `if cond { .. } else if cond { .. } else { .. }`
#[derive(Debug)]
pub(crate) struct IfNode {
    pub(crate) condition: syn::Expr,
    pub(crate) then_branch: Vec<Node<ControlFlow>>,
    pub(crate) else_branch: Option<Box<ElseBranch>>,
}

/// The tail of an `if`: either another `if` or a final block.
#[derive(Debug)]
pub(crate) enum ElseBranch {
    If(Box<IfNode>),
    Block(Vec<Node<ControlFlow>>),
}

/// `for pat in expr { .. }`
#[derive(Debug)]
pub(crate) struct ForNode {
    pub(crate) pat: syn::Pat,
    pub(crate) expr: syn::Expr,
    pub(crate) body: Vec<Node<ControlFlow>>,
}

/// `match expr { pat if guard => .. , .. }`
#[derive(Debug)]
pub(crate) struct MatchNode {
    pub(crate) expr: syn::Expr,
    pub(crate) arms: Vec<MatchArm>,
}

/// One `match` arm whose right-hand side is a list of nodes.
#[derive(Debug)]
pub(crate) struct MatchArm {
    pub(crate) pat: syn::Pat,
    pub(crate) guard: Option<syn::Expr>,
    pub(crate) body: Vec<Node<ControlFlow>>,
}

impl CustomNode for ControlFlow {
    fn peek_element(input: ParseStream) -> bool {
        input.peek(Token![if]) || input.peek(Token![for]) || input.peek(Token![match])
    }
}

impl ParseRecoverable for ControlFlow {
    fn parse_recoverable(parser: &mut RecoverableContext, input: ParseStream) -> Option<Self> {
        parser.parse_mixed_fn(input, |parser, input| {
            if input.peek(Token![if]) {
                Ok(ControlFlow::If(Box::new(parse_if(parser, input)?)))
            } else if input.peek(Token![for]) {
                Ok(ControlFlow::For(Box::new(parse_for(parser, input)?)))
            } else {
                Ok(ControlFlow::Match(Box::new(parse_match(parser, input)?)))
            }
        })
    }
}

/// Parse the nodes inside a `{ .. }` block.
fn parse_children(
    parser: &mut RecoverableContext,
    input: ParseStream,
) -> syn::Result<Vec<Node<ControlFlow>>> {
    let content;
    braced!(content in input);
    let mut nodes = Vec::new();
    while !content.is_empty() {
        let node = parser
            .parse_recoverable::<Node<ControlFlow>>(&content)
            .ok_or_else(|| syn::Error::new(content.span(), "egui-react: expected an rsx! node"))?;
        nodes.push(node);
    }
    Ok(nodes)
}

fn parse_if(parser: &mut RecoverableContext, input: ParseStream) -> syn::Result<IfNode> {
    input.parse::<Token![if]>()?;
    let condition = syn::Expr::parse_without_eager_brace(input)?;
    let then_branch = parse_children(parser, input)?;
    let else_branch = if input.peek(Token![else]) {
        input.parse::<Token![else]>()?;
        Some(Box::new(if input.peek(Token![if]) {
            ElseBranch::If(Box::new(parse_if(parser, input)?))
        } else {
            ElseBranch::Block(parse_children(parser, input)?)
        }))
    } else {
        None
    };
    Ok(IfNode {
        condition,
        then_branch,
        else_branch,
    })
}

fn parse_for(parser: &mut RecoverableContext, input: ParseStream) -> syn::Result<ForNode> {
    input.parse::<Token![for]>()?;
    let pat = syn::Pat::parse_multi_with_leading_vert(input)?;
    input.parse::<Token![in]>()?;
    let expr = syn::Expr::parse_without_eager_brace(input)?;
    let body = parse_children(parser, input)?;
    Ok(ForNode { pat, expr, body })
}

fn parse_match(parser: &mut RecoverableContext, input: ParseStream) -> syn::Result<MatchNode> {
    input.parse::<Token![match]>()?;
    let expr = syn::Expr::parse_without_eager_brace(input)?;

    let content;
    braced!(content in input);

    let mut arms = Vec::new();
    while !content.is_empty() {
        let pat = syn::Pat::parse_multi_with_leading_vert(&content)?;
        let guard = if content.peek(Token![if]) {
            content.parse::<Token![if]>()?;
            Some(content.parse::<syn::Expr>()?)
        } else {
            None
        };
        content.parse::<Token![=>]>()?;
        // The right-hand side is either `{ nodes }` or a single node.
        let body = if content.peek(token::Brace) {
            parse_children(parser, &content)?
        } else {
            let node = parser
                .parse_recoverable::<Node<ControlFlow>>(&content)
                .ok_or_else(|| {
                    syn::Error::new(
                        content.span(),
                        "egui-react: a match arm must be one element or `{ .. }`",
                    )
                })?;
            vec![node]
        };
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
        arms.push(MatchArm { pat, guard, body });
    }
    Ok(MatchNode { expr, arms })
}

// `ToTokens` is only used by rstml's recovery path and by IDE tooling; the real
// expansion goes through `rsx::expand`. Emitting the source shape is enough.

impl ToTokens for ControlFlow {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            ControlFlow::If(node) => node.to_tokens(tokens),
            ControlFlow::For(node) => node.to_tokens(tokens),
            ControlFlow::Match(node) => node.to_tokens(tokens),
        }
    }
}

impl ToTokens for IfNode {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let condition = &self.condition;
        let then_branch = &self.then_branch;
        let else_branch = self.else_branch.as_deref().map(|branch| match branch {
            ElseBranch::If(node) => quote!(else #node),
            ElseBranch::Block(nodes) => quote!(else { #(#nodes)* }),
        });
        tokens.extend(quote! {
            if #condition { #(#then_branch)* } #else_branch
        });
    }
}

impl ToTokens for ForNode {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let (pat, expr, body) = (&self.pat, &self.expr, &self.body);
        tokens.extend(quote! { for #pat in #expr { #(#body)* } });
    }
}

impl ToTokens for MatchNode {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let expr = &self.expr;
        let arms = self.arms.iter().map(|arm| {
            let (pat, body) = (&arm.pat, &arm.body);
            let guard = arm.guard.as_ref().map(|guard| quote!(if #guard));
            quote! { #pat #guard => { #(#body)* } }
        });
        tokens.extend(quote! { match #expr { #(#arms)* } });
    }
}
