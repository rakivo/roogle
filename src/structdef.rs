use std::fmt::{Debug, Formatter};

use rayon::prelude::*;
use proc_macro2::TokenStream;
use syn::{
    Type,
    Ident,
    Token,
    token::{Brace, Paren},
    parse::{Parse, ParseStream}
};

use crate::loc::Loc;
use crate::{skip_tokens, to_static_str};

pub enum FieldsKind {
    Named,
    Unnamed,
    Unit
}

impl From::<&syn::Fields> for FieldsKind {
    fn from(fields: &syn::Fields) -> Self {
        match fields {
            syn::Fields::Named(..) => Self::Named,
            syn::Fields::Unnamed(..) => Self::Unnamed,
            syn::Fields::Unit => Self::Unit
        }
    }
}

pub struct Field {
    pub name: Option::<&'static str>,
    pub ty: &'static str
}

pub type FieldsNamed = Vec::<Field>;
pub type FieldsUnnamed = Vec::<Field>;

pub enum Fields {
    Named(FieldsNamed),
    Unnamed(FieldsUnnamed),
    Unit
}

impl Fields {
    pub fn iter(&self) -> Box::<dyn Iterator<Item = &Field> + '_> {
        match self {
            Self::Named(ref fields) | Self::Unnamed(ref fields) => Box::new(fields.iter()),
            _ => Box::new(std::iter::empty())
        }
    }

    pub fn par_iter(&self) -> Option::<impl ParallelIterator<Item = &Field>> {
        match self {
            Self::Named(ref fields) | Self::Unnamed(ref fields) => Some(fields.par_iter()),
            Self::Unit => None
        }
    }
}

pub struct StructDef {
    pub name: Option::<&'static str>,
    pub is_tup: bool,
    pub fields: Fields
}

pub type StructDefs<'a> = Vec::<(Loc::<'a>, StructDef)>;

pub fn parse_optional_named_field(input: ParseStream) -> syn::Result::<Field> {
    let name = if input.peek2(Token![:]) && !input.peek3(Token![:]) {
        let name = input.parse::<Ident>().unwrap();
        skip_tokens!(input, :);
        Some(name)
    } else {
        None
    };

    let ty = if input.is_empty() {
        Type::Verbatim(TokenStream::new())
    } else {
        input.parse::<Type>().unwrap()
    };

    let ty = to_static_str(&ty);
    let f = if let Some(name) = name {
        Field {
            name: Some(to_static_str(&name)),
            ty,
        }
    } else {
        Field {name: None, ty}
    };

    Ok(f)
}

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
                let field = parse_optional_named_field(&content).unwrap();
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
                    ty: to_static_str(&input.parse::<Type>()?)
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
        let mut fields = Vec::with_capacity(structdef.fields.len());
        let kind = FieldsKind::from(&structdef.fields);
        structdef.fields.into_iter().for_each(|f| {
            let name = f.ident.as_ref().map(to_static_str);
            let ty = to_static_str(&f.ty);
            let f = Field {name, ty};
            fields.push(f);
        });
        let fields = match kind {
            FieldsKind::Named => Fields::Named(fields),
            FieldsKind::Unnamed => Fields::Unnamed(fields),
            FieldsKind::Unit => Fields::Unit
        };
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
            let ty = fi.ty;
            if let Some(ref name) = fi.name {
                writeln!(f, "{name}: {ty},")?
            } else {
                writeln!(f, "[null]: {ty},")?
            }
        }
        Ok(())
    }
}
