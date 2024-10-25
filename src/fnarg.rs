use std::fmt::{Debug, Formatter};

use quote::{quote, ToTokens};
use proc_macro2::TokenStream;
use syn::{
    Type,
    Token,
    parse::{Parse, ParseStream}
};

use crate::skip_tokens;

pub struct FnArg {
    pub name: Option::<TokenStream>,
    pub ty: Option::<Box::<TokenStream>>
}

impl Debug for FnArg {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let ty = self.ty.to_token_stream();
        if let Some(ref name) = self.name {
            write!(f, "\"{name}\": {ty}")
        } else {
            write!(f, "[null]: {ty}")
        }
    }
}

impl ToTokens for FnArg {
    #[inline(always)]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ref name = self.name;
        let ref ty = self.ty;
        tokens.extend(quote!(#name: #ty));
    }
}

impl Parse for FnArg {
    fn parse(input: ParseStream) -> syn::Result::<Self> {
        let name = input.parse::<Type>().unwrap();
        let (name, ty) = if input.peek(Token![:]) {
            skip_tokens!(input, :);
            (Some(name.into_token_stream()), Some(input.parse().unwrap()))
        } else {
            (None, Some(name))
        };

        let ty = Some(Box::new(ty.into_token_stream()));
        Ok(FnArg{name, ty})
    }
}
