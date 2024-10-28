use rayon::prelude::*;
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

use crate::{loc::Loc, print_results};
use crate::fields::*;
use crate::enummap::*;
use crate::{skip_tokens, to_static_str};

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
    pub fn search_enum_def<'a>(query: &'a EnumDef, enums: &'a EnumDefs) {
        let cache = EnumMap::new(enums);

        let mut vnames = Vec::new();
        let mut name_candidates = query.variants
            .iter()
            .flat_map(|variant| {
                if let Some(vname) = variant.name {
                    vnames.push(vname);
                }
                variant.fields.iter()
            }).filter_map(|f| f.name)
            .flat_map(|name| cache.name_map.get(name))
            .flatten()
            .collect::<Vec<_>>();

        if let Some(name) = query.name {
            if let Some(result) = cache.name_map.get(name) {
                name_candidates.par_extend(result);
            }
        }

        name_candidates.par_extend(vnames.into_par_iter().filter_map(|name| cache.name_map.get(name)).flatten());

        let type_candidates = query.variants
            .iter()
            .flat_map(|variant| variant.fields.iter())
            .filter_map(|f| f.ty)
            .flat_map(|ty| cache.type_map.get(ty))
            .flatten()
            .collect::<Vec<_>>();

        let results = name_candidates
            .into_iter()
            .chain(type_candidates)
            .map(std::ops::Deref::deref)
            .collect();

        print_results(&results);
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
