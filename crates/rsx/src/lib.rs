//! `rsx! { <tag attr={expr} bare>{child}</tag> }` expands to a plain
//! builder-chain expression: `tag().attr(expr).bare().child(child)`.
//!
//! Not tied to gpui or any other framework — it only knows how to turn tags
//! into `path()` calls, attributes into `.method(args)` calls, and children
//! into `.child(..)` / `.children(..)` calls. Grammar:
//!
//! ```text
//! <path attr* />                       self-closing, no children
//! <path attr*> node* </path>           children between open/close tags
//!
//! attr := ident                        -> .ident()
//!       | ident = literal              -> .ident(literal)
//!       | ident = { expr, expr, .. }   -> .ident(expr, expr, ..)
//!
//! node := <element>                    -> nested .child(element)
//!       | { expr }                     -> .child(expr)
//!       | { ..expr }                   -> .children(expr)
//!       | "text"                       -> .child("text")
//! ```
//!
//! `ident = { a, b }` (comma-separated, not a single-expr restriction) so
//! multi-argument builder methods work, e.g. `on_click={MouseButton::Left, handler}`
//! for `.on_click(MouseButton::Left, handler)`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{braced, Expr, Ident, Lit, LitStr, Path, Token};

struct Element {
    path: Path,
    attrs: Vec<Attr>,
    children: Vec<Node>,
}

struct Attr {
    name: Ident,
    args: Option<Punctuated<Expr, Token![,]>>,
}

enum Node {
    Element(Element),
    Block(Expr),
    Spread(Expr),
    Text(LitStr),
}

impl Parse for Attr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = Ident::parse_any(input)?;
        if !input.peek(Token![=]) {
            return Ok(Attr { name, args: None });
        }
        input.parse::<Token![=]>()?;

        if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            let args = Punctuated::<Expr, Token![,]>::parse_terminated(&content)?;
            Ok(Attr {
                name,
                args: Some(args),
            })
        } else {
            let lit: Lit = input.parse()?;
            let mut args = Punctuated::new();
            args.push(Expr::Lit(syn::ExprLit { attrs: vec![], lit }));
            Ok(Attr {
                name,
                args: Some(args),
            })
        }
    }
}

fn parse_child(input: ParseStream) -> syn::Result<Node> {
    if input.peek(Token![<]) {
        return Ok(Node::Element(input.parse()?));
    }
    if input.peek(syn::token::Brace) {
        let content;
        braced!(content in input);
        if content.peek(Token![..]) {
            content.parse::<Token![..]>()?;
            let expr: Expr = content.parse()?;
            return Ok(Node::Spread(expr));
        }
        let expr: Expr = content.parse()?;
        return Ok(Node::Block(expr));
    }
    if input.peek(LitStr) {
        return Ok(Node::Text(input.parse()?));
    }
    Err(input.error("expected `<element>`, `{expr}`, `{..expr}`, or a string literal"))
}

impl Parse for Element {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![<]>()?;
        let path: Path = input.parse()?;

        let mut attrs = Vec::new();
        while !input.peek(Token![/]) && !input.peek(Token![>]) {
            attrs.push(input.parse::<Attr>()?);
        }

        if input.peek(Token![/]) {
            input.parse::<Token![/]>()?;
            input.parse::<Token![>]>()?;
            return Ok(Element {
                path,
                attrs,
                children: Vec::new(),
            });
        }
        input.parse::<Token![>]>()?;

        let mut children = Vec::new();
        while !(input.peek(Token![<]) && input.peek2(Token![/])) {
            if input.is_empty() {
                return Err(input.error("unexpected end of input, expected a closing tag"));
            }
            children.push(parse_child(input)?);
        }

        input.parse::<Token![<]>()?;
        input.parse::<Token![/]>()?;
        let close_path: Path = input.parse()?;
        input.parse::<Token![>]>()?;

        let open_name = path.segments.last().unwrap().ident.clone();
        let close_name = close_path.segments.last().unwrap().ident.clone();
        if open_name != close_name {
            return Err(syn::Error::new_spanned(
                close_path,
                format!("closing tag `</{close_name}>` does not match opening tag `<{open_name}>`"),
            ));
        }

        Ok(Element {
            path,
            attrs,
            children,
        })
    }
}

impl Element {
    fn expand(&self) -> TokenStream2 {
        let path = &self.path;
        let mut expr = quote! { #path() };

        for attr in &self.attrs {
            let name = &attr.name;
            expr = match &attr.args {
                Some(args) => {
                    let args = args.iter();
                    quote! { (#expr).#name(#(#args),*) }
                }
                None => quote! { (#expr).#name() },
            };
        }

        for child in &self.children {
            expr = match child {
                Node::Element(el) => {
                    let child_tokens = el.expand();
                    quote! { (#expr).child(#child_tokens) }
                }
                Node::Block(e) => quote! { (#expr).child(#e) },
                Node::Spread(e) => quote! { (#expr).children(#e) },
                Node::Text(lit) => quote! { (#expr).child(#lit) },
            };
        }

        expr
    }
}

/// See the crate-level docs for the grammar.
#[proc_macro]
pub fn rsx(input: TokenStream) -> TokenStream {
    let element = syn::parse_macro_input!(input as Element);
    element.expand().into()
}
