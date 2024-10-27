use syn::{Token, parse::{Parse, ParseStream}};

use crate::enumdef::EnumDef;
use crate::fnsig::FnSignature;
use crate::structdef::StructDef;

#[derive(Debug)]
pub enum Item {
    EnumDef(EnumDef),
    StructDef(StructDef),
    FnSignature(FnSignature)
}

impl Parse for Item {
    fn parse(input: ParseStream) -> syn::Result::<Self> {
        if input.parse::<Token![fn]>().is_ok() {
            Ok(Item::FnSignature(input.parse()?))
        } else if input.parse::<Token![struct]>().is_ok() {
            Ok(Item::StructDef(input.parse()?))
        } else if input.parse::<Token![enum]>().is_ok() {
            Ok(Item::EnumDef(input.parse()?))
        } else {
            panic!("unexpected input: {input}, expected `fn` or `struct` at the beginning")
        }
    }
}
