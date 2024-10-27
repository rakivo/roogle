use std::fmt::{Debug, Formatter};

use syn::{
    Type,
    Ident,
    Token,
    token::{Brace, Paren},
    parse::{Parse, ParseStream}
};

use crate::loc::Loc;
use crate::fields::*;
use crate::{skip_tokens, to_static_str};

pub struct StructDef {
    pub name: Option::<&'static str>,
    pub is_tup: bool,
    pub fields: Fields
}

pub type StructDefs<'a> = Vec::<(Loc::<'a>, StructDef)>;

impl Parse for StructDef {
    fn parse(input: ParseStream) -> syn::Result::<Self> {
        skip_tokens!(input, struct);
        let name = if let Ok(ident) = input.parse::<Ident>() {
            Some(to_static_str(&ident))
        } else {
            None
        };
        let lookahead = input.lookahead1();
        if lookahead.peek(Brace) {
            let content;
            syn::braced!(content in input);
            let mut fields = Vec::new();
            loop {
                if content.is_empty() { break }
                let field = parse_optionaly_named_field(&content).unwrap();
                fields.push(field);
                if content.is_empty() { break }
                skip_tokens!(input, ,);
            }
            let fields = Fields::Named(fields);
            Ok(StructDef{name, is_tup: false, fields})
        } else if lookahead.peek(Paren) {
            let content;
            syn::parenthesized!(content in input);
            let mut fields = Vec::new();
            loop {
                if content.is_empty() { break }
                let field = Field {
                    name: None,
                    ty: Some(to_static_str(&input.parse::<Type>()?))
                };
                fields.push(field);
                if content.is_empty() { break }
                skip_tokens!(input, ,);
            }
            let fields = Fields::Unnamed(fields);
            Ok(StructDef{name, is_tup: true, fields})
        } else if lookahead.peek(syn::Token![;]) {
            input.parse::<syn::Token![;]>().unwrap();
            let fields = Fields::Unit;
            Ok(StructDef{name, is_tup: false, fields})
        } else {
            Err(lookahead.error())
        }
    }
}

impl From::<syn::ItemStruct> for StructDef {
    fn from(structdef: syn::ItemStruct) -> Self {
        let name = Some(to_static_str(&structdef.ident));
        let is_tup = matches!(structdef.fields, syn::Fields::Unnamed(_));
        let fields = Fields::from(structdef.fields);
        Self {name, is_tup, fields}
    }
}

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
            let ty = fi.ty.unwrap_or_default();
            if let Some(ref name) = fi.name {
                writeln!(f, "{name}: {ty},")?
            } else {
                writeln!(f, "[null]: {ty},")?
            }
        }
        Ok(())
    }
}
