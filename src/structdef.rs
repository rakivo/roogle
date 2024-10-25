use std::hash::{Hash, Hasher};
use std::fmt::{Debug, Formatter};

use quote::ToTokens;
use proc_macro2::TokenStream;
use syn::{
    Type,
    Ident,
    Token,
    token::{Brace, Paren},
    parse::{Parse, ParseStream}
};

use crate::skip_tokens;

pub struct StructDef {
    pub name: Option::<String>,
    pub is_tup: bool,
    pub fields: syn::Fields
}

pub fn parse_optional_named_field(input: ParseStream) -> syn::Result::<syn::Field> {
    let (ident, colon_token) = if input.peek2(Token![:]) && !input.peek3(Token![:]) {
        let ident = input.parse::<Ident>().unwrap();
        let colon_token = input.parse::<Token![:]>().unwrap();
        (Some(ident), Some(colon_token))
    } else {
        (None, None)
    };

    let ty = if input.is_empty() {
        Type::Verbatim(TokenStream::new())
    } else {
        input.parse::<Type>().unwrap()
    };

    let f = syn::Field {
        attrs: Vec::new(),
        vis: syn::Visibility::Inherited,
        ident,
        colon_token,
        ty,
        mutability: syn::FieldMutability::None
    };

    Ok(f)
}

impl Parse for StructDef {
    fn parse(input: ParseStream) -> syn::Result::<Self> {
        skip_tokens!(input, struct);
        let name: Option::<Ident> = if input.peek(Ident) {
            Some(input.parse().unwrap())
        } else {
            None
        };

        let name = name.map(|i| i.to_string());
        let lookahead = input.lookahead1();
        if lookahead.peek(Brace) {
            let content;
            let brace_token = syn::braced!(content in input);
            let fields = syn::Fields::Named(syn::FieldsNamed {
                brace_token,
                named: content.parse_terminated(parse_optional_named_field, Token![,]).unwrap(),
            });
            Ok(StructDef{name, is_tup: false, fields})
        } else if lookahead.peek(Paren) {
            let content;
            let paren_token = syn::parenthesized!(content in input);
            let fields = syn::Fields::Unnamed(syn::FieldsUnnamed {
                paren_token,
                unnamed: content.parse_terminated(syn::Field::parse_unnamed, Token![,]).unwrap()
            });
            Ok(StructDef{name, is_tup: true, fields})
        } else if lookahead.peek(syn::Token![;]) {
            input.parse::<syn::Token![;]>().unwrap();
            let fields = syn::Fields::Unit;
            Ok(StructDef{name, is_tup: false, fields})
        } else {
            Err(lookahead.error())
        }
    }
}

impl Hash for StructDef {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.is_tup.hash(state);
        self.fields.iter().for_each(|f| f.ty.to_token_stream().to_string().to_lowercase().hash(state))
    }
}

impl PartialEq for StructDef {
    fn eq(&self, other: &Self) -> bool {
        if self.fields.len() != other.fields.len() { return false }
        !self.fields.iter().zip(other.fields.iter()).any(|(s1, s2)| {
            s1.ty.to_token_stream().to_string().to_lowercase() != s2.ty.to_token_stream().to_string().to_lowercase()
        })
    }
}

impl Eq for StructDef {}

impl Debug for StructDef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "name: ")?;
        if let Some(ref name) = self.name {
            writeln!(f, "{name},")?
        } else {
            writeln!(f, "[null],")?
        }
        writeln!(f, "is_tup: {is_tup},", is_tup = self.is_tup)?;
        for fi in self.fields.iter() {
            let ty = fi.ty.to_token_stream().to_string();
            if let Some(ref ident) = fi.ident {
                let name = ident.to_string();
                writeln!(f, "{name}: {ty},")?
            } else {
                writeln!(f, "[null]: {ty},")?
            }
        }
        Ok(())
    }
}
