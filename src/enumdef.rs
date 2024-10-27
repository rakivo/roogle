use syn::{
    Type,
    Ident,
    Token,
    token::{Brace, Paren},
    parse::{Parse, ParseStream}
};

use crate::{loc::Loc, skip_tokens};
use crate::fields::*;
use crate::to_static_str;

#[derive(Debug)]
pub struct Variant {
    pub name: Option::<&'static str>,
    pub fields: Fields
}

#[derive(Debug)]
pub struct EnumDef {
    pub name: Option::<&'static str>,
    pub variants: Vec::<Variant>
}

impl From::<syn::ItemEnum> for EnumDef {
    fn from(e: syn::ItemEnum) -> Self {
        let name = Some(to_static_str(&e.ident));
        let variants = e.variants.into_iter().map(|v| {
            Variant {
                name: Some(to_static_str(&v.ident)),
                fields: Fields::from(v.fields),
            }
        }).collect();
        Self {name, variants}
    }
}

impl Parse for EnumDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        skip_tokens!(input, enum);
        
        let name = if let Ok(ident) = input.parse::<Ident>() {
            Some(to_static_str(&ident))
        } else {
            None
        };

        let content;
        syn::braced!(content in input);
        
        let mut variants = Vec::new();

        // Parse each variant in the enum
        while !content.is_empty() {
            // Parse the variant name
            let variant_name = to_static_str(&content.parse::<Ident>().unwrap());

            // Determine the type of fields for this variant
            let lookahead = content.lookahead1();
            let fields = if lookahead.peek(Brace) {
                // Parse named fields
                let inner_content;
                syn::braced!(inner_content in content);
                let mut fields = Vec::new();
                while !inner_content.is_empty() {
                    let field = parse_optionaly_named_field(&inner_content).unwrap();
                    fields.push(field);
                    if inner_content.is_empty() { break; }
                    inner_content.parse::<Token![,]>().unwrap();
                }
                Fields::Named(fields)
            } else if lookahead.peek(Paren) {
                // Parse unnamed fields
                let inner_content;
                syn::parenthesized!(inner_content in content);
                let mut fields = Vec::new();
                while !inner_content.is_empty() {
                    let ty = Some(to_static_str(&inner_content.parse::<Type>().unwrap()));
                    fields.push(Field { name: None, ty });
                    if inner_content.is_empty() { break; }
                    inner_content.parse::<Token![,]>().unwrap();
                }
                Fields::Unnamed(fields)
            } else {
                // Unit variant
                Fields::Unit
            };

            // Add the parsed variant to the list
            variants.push(Variant { name: Some(variant_name), fields });

            // Parse optional comma after each variant
            if content.peek(Token![,]) {
                content.parse::<Token![,]>().unwrap();
            }
        }

        Ok(EnumDef { name, variants })
    }
}

pub type EnumDefs<'a> = Vec::<(Loc<'a>, EnumDef)>;
