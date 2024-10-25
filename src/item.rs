use std::path::PathBuf;
use std::collections::HashMap;

use syn::{Token, parse::{Parse, ParseStream}};

use crate::loc::Loc;
use crate::fnsig::FnSignature;
use crate::structdef::StructDef;

#[derive(Eq, Hash, Debug, PartialEq)]
pub enum Item {
    StructDef(StructDef),
    FnSignature(FnSignature)
}

impl Parse for Item {
    fn parse(input: ParseStream) -> syn::Result::<Self> {
        if input.parse::<Token![fn]>().is_ok() {
            Ok(Item::FnSignature(input.parse::<FnSignature>()?))
        } else if input.parse::<Token![struct]>().is_ok() {
            Ok(Item::StructDef(input.parse::<StructDef>()?))
        } else {
            panic!("unexpected input: {input}, expected `fn` or `struct` at the beginning")
        }
    }
}

pub type ItemMap<'a> = HashMap::<Item, Loc::<'a>>;
pub type FileMap<'a> = HashMap::<&'a PathBuf, ItemMap<'a>>;
