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
    spanned::Spanned,
    token::{Paren, Brace},
    punctuated::Punctuated,
    parse::{Parse, ParseStream},
};

mod dir_rec;
use dir_rec::*;

macro_rules! skip_tokens {
    ($content: expr, $($t: tt), *) => {
        $(_ = $content.parse::<Token![$t]>();)*
    };
}

enum ReturnType {
    Default,
    Type(Box::<Type>)
}

impl ToTokens for ReturnType {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        *tokens = self.to_token_stream()
    }

    fn to_token_stream(&self) -> TokenStream {
        match self {
            Self::Default => TokenStream::new(),
            Self::Type(ty) => if matches!(**ty, Type::Tuple(ref t) if t.elems.is_empty()) {
                TokenStream::new()                    
            } else {
                ty.to_token_stream()
            }
        }
    }

    #[inline]
    fn into_token_stream(self) -> TokenStream
    where
        Self: Sized
    {
        self.to_token_stream()
    }
}

struct FnArg {
    name: Option::<TokenStream>,
    ty: Option::<Box::<TokenStream>>
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
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let ty = &self.ty;
        tokens.extend(quote!(#name: #ty));
    }
}

struct StructDef {
    name: Option::<String>,
    is_tup: bool,
    fields: syn::Fields,
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
            let ty = fi.ty.to_token_stream().to_string();
            if let Some(ref ident) = fi.ident {
                let name = ident.to_string();
                writeln!(f, "{name}: {ty},")?
            } else {
                writeln!(f, "[null]: {ty},")?
            }
        }
        Ok(())
    }
}

fn parse_optional_named_field(input: ParseStream) -> syn::Result::<syn::Field> {
    let (ident, colon_token) = if input.peek2(Token![:]) && !input.peek3(Token![:]) {
        let ident = input.parse::<Ident>().unwrap();
        let colon_token = input.parse::<Token![:]>().unwrap();
        (Some(ident), Some(colon_token))
    } else {
        (None, None)
    };

    let ty = if input.is_empty() {
        Type::Verbatim(TokenStream::new())
    } else {
        input.parse::<Type>().unwrap()
    };

    Ok(syn::Field {
        attrs: Vec::new(),
        vis: syn::Visibility::Inherited,
        ident,
        colon_token,
        ty,
        mutability: syn::FieldMutability::None
    })
}

impl Parse for StructDef {
    fn parse(input: ParseStream) -> syn::Result::<Self> {
        skip_tokens!(input, struct);
        let name: Option::<Ident> = if input.peek(Ident) {
            Some(input.parse().unwrap())
        } else {
            None
        };

        let name = name.map(|i| i.to_string());
        let lookahead = input.lookahead1();
        if lookahead.peek(Brace) {
            let content;
            let _brace_token = syn::braced!(content in input);
            let fields = syn::Fields::Named(syn::FieldsNamed {
                brace_token: _brace_token,
                named: content.parse_terminated(parse_optional_named_field, Token![,]).unwrap(),
            });
            
            Ok(StructDef {name, is_tup: false, fields})
        } else if lookahead.peek(Paren) {
            let content;
            let _paren_token = syn::parenthesized!(content in input);
            let fields = syn::Fields::Unnamed(syn::FieldsUnnamed {
                paren_token: _paren_token,
                unnamed: content.parse_terminated(syn::Field::parse_unnamed, Token![,]).unwrap()
            });

            Ok(StructDef {name, is_tup: true, fields})
        } else if lookahead.peek(syn::Token![;]) {
            input.parse::<syn::Token![;]>().unwrap();
            let fields = syn::Fields::Unit;

            Ok(StructDef {name, is_tup: false, fields})
        } else {
            Err(lookahead.error())
        }
    }
}

impl PartialEq for StructDef {
    fn eq(&self, other: &Self) -> bool {
        if self.fields.len() != other.fields.len() { return false }
        !self.fields.iter().zip(other.fields.iter()).any(|(s1, s2)| {
            s1.ty.to_token_stream().to_string() != s2.ty.to_token_stream().to_string()
        })
    }
}

impl Eq for StructDef {}

impl Hash for StructDef {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.is_tup.hash(state);
        self.fields.iter().for_each(|f| f.ty.to_token_stream().to_string().hash(state))
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
                let ty = Some(Box::new(ty.to_token_stream()));
                if let Pat::Ident(PatIdent { ident, .. }) = *pat {
                    let name = Some(ident.into_token_stream());
                    Some(FnArg{name, ty})
                } else {
                    let name = None;
                    Some(FnArg{name, ty})
                }
            }
        }
    }).collect()
}

#[inline]
fn signature_get_output(output: syn::ReturnType) -> ReturnType {
    match output {
        syn::ReturnType::Default => ReturnType::Default,
        syn::ReturnType::Type(.., ty) => ReturnType::Type(ty),
    }
}

impl From::<Signature> for FnSignature {
    #[inline]
    fn from(syn_sig: Signature) -> Self {
        let name = Some(syn_sig.ident.to_string());
        let inputs = signature_get_inputs(syn_sig.inputs);
        let output = signature_get_output(syn_sig.output);
        FnSignature { name, inputs, output }
    }
}

impl From::<ImplItemFn> for FnSignature {
    #[inline]
    fn from(item: ImplItemFn) -> Self {
        let name = Some(item.sig.ident.to_string());
        let inputs = signature_get_inputs(item.sig.inputs);
        let output = signature_get_output(item.sig.output);
        FnSignature { name, inputs, output }
    }
}

#[inline]
fn inputs_to_string(inputs: &Vec::<FnArg>) -> Vec::<String> {
    inputs.iter().map(|FnArg{ty, ..}| quote::quote!(#ty).to_string()).collect()
}

impl Hash for FnSignature {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        inputs_to_string(&self.inputs).hash(state);
        self.output.to_token_stream().to_string().hash(state);
    }
}

impl PartialEq for FnSignature {
    fn eq(&self, other: &Self) -> bool {
        if self.inputs.len() != other.inputs.len() { return false }

        let self_input_types = inputs_to_string(&self.inputs);
        let other_input_types = inputs_to_string(&other.inputs);

        if self_input_types != other_input_types { return false }

        let self_output = self.output.to_token_stream().to_string();
        let other_output = other.output.to_token_stream().to_string();

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

        Ok(FnSignature {
            name: name.map(|i| i.to_string()),
            inputs,
            output: signature_get_output(input.parse::<syn::ReturnType>().unwrap()),
        })
    }
}

//            file_path, line, column
struct Loc<'a>(&'a PathBuf, usize, usize);

impl<'a > Loc<'a> {
    #[inline]
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

#[derive(Eq, Hash, Debug, PartialEq)]
enum Item {
    StructDef(StructDef),
    FnSignature(FnSignature)
}

impl Parse for Item {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.parse::<Token![fn]>().is_ok() {
            Ok(Item::FnSignature(input.parse::<FnSignature>()?))
        } else if input.parse::<Token![struct]>().is_ok() {
            Ok(Item::StructDef(input.parse::<StructDef>()?))
        } else {
            panic!("unexpected input: {input}")
        }
    }
}

type ItemMap<'a> = HashMap::<Item, Loc::<'a>>;
type FileMap<'a> = HashMap::<&'a PathBuf, ItemMap<'a>>;

#[allow(unused)]
fn get_ident(item: &syn::Item) -> Option::<String> {
    match item {
        syn::Item::Const(_const) => Some(_const.ident.to_string()),
        syn::Item::Enum(_enum) => Some(_enum.ident.to_string()),
        syn::Item::ExternCrate(extern_crate) => Some(extern_crate.ident.to_string()),
        syn::Item::Fn(_fn) => Some(_fn.sig.ident.to_string()),
        syn::Item::ForeignMod(..) => None,
        syn::Item::Impl(..) => None,
        syn::Item::Macro(_macro) => if let Some(ref ident) = _macro.ident {
            Some(ident.to_string())
        } else { None },
        syn::Item::Mod(_mod) => Some(_mod.ident.to_string()),
        syn::Item::Static(_static) => Some(_static.ident.to_string()),
        syn::Item::Struct(_struct) => Some(_struct.ident.to_string()),
        syn::Item::Trait(_trait) => Some(_trait.ident.to_string()),
        syn::Item::TraitAlias(_trait_alias) => Some(_trait_alias.ident.to_string()),
        syn::Item::Type(_type) => Some(_type.ident.to_string()),
        syn::Item::Union(_union) => Some(_union.ident.to_string()),
        syn::Item::Use(_use) => None,
        syn::Item::Verbatim(..) => None,
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
        let span = syn_item.span();
        match syn_item {
            syn::Item::Fn(f) => {
                let loc = Loc::from_span(file_path, &span);
                let sig = FnSignature::from(f.sig);
                map.insert(Item::FnSignature(sig), loc);
            }
            syn::Item::Struct(s) => {
                let loc = Loc::from_span(file_path, &span);
                let struc = StructDef {
                    name: Some(s.ident.to_string()),
                    is_tup: matches!(s.fields, syn::Fields::Unnamed(_)),
                    fields: s.fields
                };
                map.insert(Item::StructDef(struc), loc);
            }
            syn::Item::Impl(im) => {
                impl_get_fns(file_path, im).into_iter().for_each(|(sig, loc)| {
                    map.insert(Item::FnSignature(sig), loc);
                })
            }
            _ => {}
        } map
    });
    Ok(map)
}

#[inline(always)]
fn search_for_item<'a>(item: &Item, map: &'a FileMap<'a>) -> Vec::<&'a Loc<'a>> {
    map.iter().filter_map(|(.., item_map)| item_map.get(&item)).collect()
}

fn main() -> ExitCode {
    let args = env::args().collect::<Vec::<_>>();
    if args.len() < 2 {
        eprintln!("usage: <{program}> <signature>", program = args[0]);
        return ExitCode::FAILURE
    }

    let dir = DirRec::new(".");
    let contents = dir.into_iter()
        .par_bridge()
        .filter(|e| e.extension().unwrap_or_default().eq("rs"))
        .filter_map(|e| {
            read_to_string(&e).ok().map(|code| (e, code))
        }).collect::<Vec::<_>>();

    let map = contents.iter().filter_map(|(file_path, code)| {
        parse(file_path, code).ok().map(|map| (file_path, map))
    }).collect::<FileMap>();

    let item = syn::parse_str::<Item>(&args[1]).unwrap();
    search_for_item(&item, &map).iter().for_each(|loc| println!("{loc}"));

    ExitCode::SUCCESS
}

/* TODO:
    1. Search not only by types but by names too, but idk how to do that the fastest way
    2. Support lifetimes, generics.
*/
