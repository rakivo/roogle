use syn::{
    Type,
    Ident,
    Token,
    token::{Brace, Paren},
    parse::{
        discouraged::AnyDelimiter,
        Parse,
        ParseStream
    }
};

use crate::loc::Loc;
use crate::fields::*;
use crate::{Results, skip_tokens, to_static_str};

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

impl EnumDef {
    pub fn search_enum_def<'a>(query: &'a EnumDef, enums: &'a EnumDefs) -> Results<'a, 'a> {
        enums.iter()
            .filter(|(.., enum_def)| Self::matches_enum_def(query, enum_def))
            .map(|(loc, ..)| loc)
            .collect()
    }

    fn matches_enum_def(query: &EnumDef, enum_def: &EnumDef) -> bool {
        if query.name.map_or(false, |q_name| {
            enum_def.name.map_or(false, |name| name == q_name)
        }) {
            return true
        }

        query.variants.iter().any(|q_variant| {
            enum_def.variants.iter().any(|v| Self::matches_variant(q_variant, v))
        })
    }

    fn matches_variant(query: &Variant, variant: &Variant) -> bool {
        if query.name.map_or(false, |q_name| {
            variant.name.map_or(false, |name| name == q_name)
        }) {
            return true
        }

        Self::matches_fields(&query.fields, &variant.fields)
    }

    fn matches_fields(query: &Fields, fields: &Fields) -> bool {
        match (query, fields) {
            (Fields::Unit, Fields::Unit) => true,
            (Fields::Named(query_fields), Fields::Named(fields)) 
            | (Fields::Unnamed(query_fields), Fields::Unnamed(fields)) => {
                query_fields.iter().any(|q_field| {
                    fields.iter().any(|field| Self::matches_field(q_field, field))
                })
            }
            _ => false,
        }
    }

    fn matches_field(query: &Field, field: &Field) -> bool {
        if query.name.map_or(false, |q_name| {
            field.name.map_or(false, |name| name == q_name)
        }) {
            return true
        }

        query.ty.map_or(false, |q_ty| {
            field.ty.map_or(false, |ty| ty == q_ty)
        })
    }
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
    fn parse(input: ParseStream) -> syn::Result::<Self> {
        skip_tokens!(input, enum);
        
        let name = if let Ok(ident) = input.parse::<Ident>() {
            Some(to_static_str(&ident))
        } else {
            None
        };

        let content;
        syn::braced!(content in input);
        
        let mut variants = Vec::new();
        while !content.is_empty() {
            let name = {
                let name = content.parse::<Type>().as_ref().map(to_static_str).ok();
                skip_tokens!(content, ,);
                if content.is_empty() {
                    variants.push(Variant {
                        name: None,
                        fields: Fields::Unnamed(vec![Field {
                            name: None,
                            ty: name
                        }])
                    });
                    break
                } else {
                    name
                }
            };

            let lookahead = content.lookahead1();
            let fields = if lookahead.peek(Brace) {
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
                if content.peek2(Paren) {
                    content.parse_any_delimiter().unwrap();
                    Fields::Unit
                } else {
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
                }
            } else {
                Fields::Unnamed(vec![Field {
                    name: None,
                    ty: Some(to_static_str(&content.parse::<Type>().unwrap()))
                }])
            };

            variants.push(Variant {name, fields});

            if content.peek(Token![,]) {
                content.parse::<Token![,]>().unwrap();
            }
        }

        Ok(EnumDef { name, variants })
    }
}

pub type EnumDefs<'a> = Vec::<(Loc<'a>, EnumDef)>;
