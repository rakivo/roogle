use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::fs::read_to_string;
use std::hash::{Hash, Hasher};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};

use rayon::prelude::*;
use quote::{quote, ToTokens};
use proc_macro2::{Span, TokenStream};
use syn::{
    Pat,
    File,
    Type,
    Token,
    Ident,
    PatType,
    PatIdent,
    ItemImpl,
    ImplItem,
    Signature,
    ImplItemFn,
    ReturnType,
    Item as SynItem,
    spanned::Spanned,
    parse::ParseBuffer,
    punctuated::Punctuated,
    parse::{Parse, ParseStream},
};

mod dir_rec;
use dir_rec::*;

struct FnArg(TokenStream, Box::<TokenStream>);

impl Debug for FnArg {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let ref name = self.0;
        let ref ty = self.1;
        write!(f, "\"{name}\": {ty}", ty = ty.to_token_stream())
    }
}

impl ToTokens for FnArg {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.0;
        let ty = &self.1;
        tokens.extend(quote!(#name: #ty));
    }
}

struct FnSignature {
    name: Option::<String>,
    inputs: Vec::<FnArg>,
    output: ReturnType
}

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

fn signature_get_inputs(inputs: Punctuated::<syn::FnArg, Token![,]>) -> Vec::<FnArg> {
    inputs.into_iter().filter_map(|fn_arg| {
        match fn_arg {
            syn::FnArg::Receiver(..) => None,
            syn::FnArg::Typed(PatType { pat, ty, .. }) => {
                if let Pat::Ident(PatIdent { ident, .. }) = *pat {
                    Some(FnArg(ident.into_token_stream(), Box::new(ty.to_token_stream())))
                } else {
                    Some(FnArg(DEFAULT_ARG_NAME.into_token_stream(), Box::new(ty.to_token_stream())))
                }
            }
        }
    }).collect()
}

fn signature_get_output(output: ReturnType) -> ReturnType {
    match output {
        syn::ReturnType::Default => ReturnType::Default,
        syn::ReturnType::Type(_, ty) => ReturnType::Type(Token![->](Span::call_site()), ty),
    }
}

impl From::<Signature> for FnSignature {
    fn from(syn_sig: Signature) -> Self {
        let name = Some(syn_sig.ident.to_string());
        let inputs = signature_get_inputs(syn_sig.inputs);
        let output = signature_get_output(syn_sig.output);
        FnSignature { name, inputs, output }
    }
}

fn output_to_string(output: &ReturnType) -> String {
    match output {
        ReturnType::Default => DEFAULT_OUTPUT,
        ReturnType::Type(.., ty) => quote::quote!(#ty).to_string()
    }
}

impl From::<ImplItemFn> for FnSignature {
    fn from(item: ImplItemFn) -> Self {
        let name = Some(item.sig.ident.to_string());
        let inputs = signature_get_inputs(item.sig.inputs);
        let output = signature_get_output(item.sig.output);
        FnSignature { name, inputs, output }
    }
}

impl Hash for FnSignature {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for FnArg(.., ty) in self.inputs.iter() {
            let type_string = quote::quote!(#ty).to_string();
            type_string.hash(state);
        }
        let output_string = output_to_string(&self.output);
        output_string.hash(state);
    }
}

const DEFAULT_OUTPUT: String = String::new();
const DEFAULT_ARG_NAME: String = String::new();

impl PartialEq for FnSignature {
    fn eq(&self, other: &Self) -> bool {
        if self.inputs.len() != other.inputs.len() { return false }

        fn inputs_to_string(inputs: &Vec::<FnArg>) -> Vec::<String> {
            inputs.iter().map(|ty| quote::quote!(#ty).to_string()).collect()
        }

        let self_input_types = inputs_to_string(&self.inputs);
        let other_input_types = inputs_to_string(&other.inputs);

        if self_input_types != other_input_types { return false }

        let self_output = output_to_string(&self.output);
        let other_output = output_to_string(&other.output);

        self_output == other_output
    }
}

impl Eq for FnSignature {}

fn skip_self(stream: &ParseBuffer) {
    _ = stream.parse::<Token![&]>();
    _ = stream.parse::<Token![self]>();
    _ = stream.parse::<Token![,]>();
}

impl Parse for FnSignature {
    fn parse(input: ParseStream) -> syn::Result::<Self> {
        input.parse::<Token![fn]>()?;
        
        let name = if input.peek(Ident) {
            Some(unsafe { input.parse::<Ident>().unwrap_unchecked() }.to_string())
        } else {
            None
        };

        let content;
        syn::parenthesized!(content in input);

        let mut inputs = Vec::new();
        while !content.is_empty() {
            let arg_name = if let Ok(ident) = content.parse::<Ident>() {
                if content.peek(Token![:]) {
                    _ = content.parse::<Token![:]>();
                    _ = content.parse::<Token![,]>();
                } ident.into_token_stream()
            } else {
                skip_self(&content);
                continue
            };

            let fnarg = if content.is_empty() {
                FnArg(DEFAULT_ARG_NAME.into_token_stream(), Box::new(arg_name))
            } else {
                let ty = content.parse::<Type>()?;
                FnArg(arg_name, Box::new(ty.into_token_stream()))
            };

            inputs.push(fnarg);
            
            _ = content.parse::<Token![,]>();
        }

        let output = if input.peek(Token![->]) {
            input.parse::<Token![->]>()?;
            ReturnType::Type(Token![->](Span::call_site()), Box::new(input.parse()?))
        } else {
            ReturnType::Default
        };

        Ok(FnSignature { name, inputs, output })
    }
}

//            file_path, line, column
struct Loc<'a>(&'a PathBuf, usize, usize);

impl<'a > Loc<'a> {
    fn from_span(file_path: &'a PathBuf, span: &Span) -> Self {
        let linecol = span.start();
        Loc(file_path, linecol.line, linecol.column)
    }
}

impl Display for Loc<'_> {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{f}:{l}:{c}", f = self.0.display(), l = self.1, c = self.2)
    }
}

impl Debug for Loc<'_> {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}

type ItemMap<'a> = HashMap::<FnSignature, Loc::<'a>>;
type FileMap<'a> = HashMap::<&'a PathBuf, ItemMap<'a>>;

#[allow(unused)]
fn get_ident(item: &SynItem) -> Option::<String> {
    match item {
        SynItem::Const(_const) => Some(_const.ident.to_string()),
        SynItem::Enum(_enum) => Some(_enum.ident.to_string()),
        SynItem::ExternCrate(extern_crate) => Some(extern_crate.ident.to_string()),
        SynItem::Fn(_fn) => Some(_fn.sig.ident.to_string()),
        SynItem::ForeignMod(..) => None,
        SynItem::Impl(..) => None,
        SynItem::Macro(_macro) => if let Some(ref ident) = _macro.ident {
            Some(ident.to_string())
        } else { None },
        SynItem::Mod(_mod) => Some(_mod.ident.to_string()),
        SynItem::Static(_static) => Some(_static.ident.to_string()),
        SynItem::Struct(_struct) => Some(_struct.ident.to_string()),
        SynItem::Trait(_trait) => Some(_trait.ident.to_string()),
        SynItem::TraitAlias(_trait_alias) => Some(_trait_alias.ident.to_string()),
        SynItem::Type(_type) => Some(_type.ident.to_string()),
        SynItem::Union(_union) => Some(_union.ident.to_string()),
        SynItem::Use(_use) => None,
        SynItem::Verbatim(..) => None,
        _ => None
    }
}

fn impl_get_fns<'a>(file_path: &'a PathBuf, im: ItemImpl) -> Vec::<(FnSignature, Loc<'a>)> {
    im.items.into_iter().filter_map(|item| {
        match item {
            ImplItem::Fn(f) => {
                let span = f.span();
                Some((FnSignature::from(f), Loc::from_span(file_path, &span)))
            },
            _ => None
        }
    }).collect()
}

fn parse<'a>(file_path: &'a PathBuf, code: &String) -> syn::Result::<ItemMap::<'a>> {
    let ast = syn::parse_str::<File>(&code)?;
    let size = ast.items.len() / 2;
    let map = ast.items.into_iter().fold(HashMap::with_capacity(size), |mut map, syn_item| {
        let span = if let SynItem::Fn(ref f) = syn_item { Some(f.span()) } else { None };
        match syn_item {
            SynItem::Fn(f) => {
                let linecol = unsafe { span.unwrap_unchecked() }.start();
                let loc = Loc(file_path, linecol.line, linecol.column);
                let sig = FnSignature::from(f.sig);
                map.insert(sig, loc);
            }
            SynItem::Impl(im) => {
                impl_get_fns(file_path, im).into_iter().for_each(|(sig, loc)| {
                    map.insert(sig, loc);
                })
            }
            _ => {}
        } map
    });
    Ok(map)
}

#[inline(always)]
fn search_for_signature<'a>(sig: &FnSignature, map: &'a FileMap<'a>) -> Vec::<&'a Loc<'a>> {
    map.iter().filter_map(|(.., item_map)| item_map.get(sig)).collect()
}

fn main() -> ExitCode {
    let args = env::args().collect::<Vec::<_>>();
    if args.len() < 2 {
        eprintln!("usage: <{program}> <signature>", program = args[0]);
        return ExitCode::FAILURE
    }

    let dir = DirRec::new("..");
    let contents = dir.into_iter()
        .par_bridge()
        .filter(|e| e.extension().unwrap_or_default().eq("rs"))
        .filter_map(|e| {
            read_to_string(&e).ok().map(|code| (e, code))
        }).collect::<Vec::<_>>();

    let map = contents.iter().filter_map(|(file_path, code)| {
        parse(file_path, code).ok().map(|map| (file_path, map))
    }).collect::<FileMap>();

    let sig = syn::parse_str::<FnSignature>(&args[1]).unwrap();
    search_for_signature(&sig, &map).iter().for_each(|loc| println!("{loc}"));

    ExitCode::SUCCESS
}
