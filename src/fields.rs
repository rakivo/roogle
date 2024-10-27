use rayon::prelude::*;
use syn::{
    Type,
    Ident,
    Token,
    parse::ParseStream
};

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

#[derive(Debug)]
pub struct Field {
    pub name: Option::<&'static str>,
    pub ty: Option::<&'static str>
}

pub type FieldsNamed = Vec::<Field>;
pub type FieldsUnnamed = Vec::<Field>;

#[derive(Debug)]
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

impl From::<syn::Fields> for Fields {
    fn from(fs: syn::Fields) -> Self {
        let kind = FieldsKind::from(&fs);
        let mut fields = Vec::with_capacity(fs.len());
        fs.into_iter().for_each(|f| {
            let name = f.ident.as_ref().map(to_static_str);
            let ty = Some(to_static_str(&f.ty));
            let f = Field {name, ty};
            fields.push(f);
        });
        match kind {
            FieldsKind::Named => Fields::Named(fields),
            FieldsKind::Unnamed => Fields::Unnamed(fields),
            FieldsKind::Unit => Fields::Unit
        }
    }
}

pub fn parse_optionaly_named_field(input: ParseStream) -> syn::Result::<Field> {
    let name = if input.peek2(Token![:]) && !input.peek3(Token![:]) {
        let name = input.parse::<Ident>().unwrap();
        skip_tokens!(input, :);
        Some(name)
    } else {
        None
    };

    let ty = if input.is_empty() {
        None
    } else {
        Some(input.parse::<Type>()?)
    };

    let ty = ty.as_ref().map(to_static_str);
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
