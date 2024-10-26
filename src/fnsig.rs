use std::hash::{Hash, Hasher};
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};

use quote::ToTokens;
use syn::{
    Ident,
    Token,
    ImplItemFn,
    Signature,
    parse::{Parse, ParseStream}
};

use crate::loc::Loc;
use crate::ReturnType;
use crate::fnarg::FnArg;
use crate::{
    skip_tokens,
    inputs_to_string,
    signature_get_output,
    signature_get_inputs
};

pub struct FnSignature {
    name: Option::<String>,
    inputs: Vec::<FnArg>,
    output: ReturnType
}

pub type FnSigs<'a> = Vec::<(Loc<'a>, FnSignature)>;
pub type FnSigMap<'a> = HashMap::<FnSignature, Loc<'a>>;

impl Debug for FnSignature {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "\nname: {:?},", self.name)?;
        write!(f, "inputs: [")?;
        if !self.inputs.is_empty() {
            writeln!(f)?;
        }
        for fnarg in self.inputs.iter() {
            writeln!(f, "    {fnarg:?},")?;
        }
        writeln!(f, "]")?;
        write!(f, "output: {}", self.output.to_token_stream())
    }
}

impl From::<Signature> for FnSignature {
    #[inline(always)]
    fn from(syn_sig: Signature) -> Self {
        FnSignature {
            name: Some(syn_sig.ident.to_string().to_lowercase()),
            inputs: signature_get_inputs(syn_sig.inputs),
            output: signature_get_output(syn_sig.output)
        }
    }
}

impl From::<ImplItemFn> for FnSignature {
    #[inline(always)]
    fn from(item: ImplItemFn) -> Self {
        FnSignature {
            name: Some(item.sig.ident.to_string().to_lowercase()),
            inputs: signature_get_inputs(item.sig.inputs),
            output: signature_get_output(item.sig.output)
        }
    }
}

impl Hash for FnSignature {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        inputs_to_string(&self.inputs).to_lowercase().hash(state);
        self.output.to_token_stream().to_string().to_lowercase().hash(state);
    }
}

impl PartialEq for FnSignature {
    fn eq(&self, other: &Self) -> bool {
        if self.inputs.len() != other.inputs.len() { return false }

        let self_input_types = inputs_to_string(&self.inputs).to_lowercase();
        let other_input_types = inputs_to_string(&other.inputs).to_lowercase();

        if self_input_types != other_input_types { return false }

        let self_output = self.output.to_token_stream().to_string().to_lowercase();
        let other_output = other.output.to_token_stream().to_string().to_lowercase();

        self_output == other_output
    }
}

impl Eq for FnSignature {}

impl Parse for FnSignature {
    fn parse(input: ParseStream) -> syn::Result::<Self> {
        skip_tokens!(input, fn);

        let name: Option::<Ident> = if input.peek(Ident) {
            Some(input.parse().unwrap())
        } else {
            None
        };

        let content;
        syn::parenthesized!(content in input);
        let mut inputs = Vec::new();
        while !content.is_empty() {
            let fn_arg = content.parse::<FnArg>()?;
            inputs.push(fn_arg);
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            } else { break }
        }

        let sig = FnSignature {
            name: name.map(|i| i.to_string()),
            inputs,
            output: signature_get_output(input.parse::<syn::ReturnType>().unwrap()),
        };

        Ok(sig)
    }
}
