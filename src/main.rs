use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::fs::read_to_string;

use quote::ToTokens;
use rayon::prelude::*;
use proc_macro2::TokenStream;
use syn::{
    Pat,
    File,
    Type,
    Token,
    PatType,
    PatIdent,
    ItemImpl,
    ImplItem,
    spanned::Spanned,
    punctuated::Punctuated
};

mod loc;
use loc::*;
mod item;
use item::*;
mod fnsig;
use fnsig::*;
mod fnarg;
use fnarg::*;
mod fields;
mod dir_rec;
use dir_rec::*;
mod enumdef;
use enumdef::*;
mod structmap;
use structmap::*;
mod structdef;
use structdef::*;

#[macro_export]
macro_rules! skip_tokens {
    ($content: expr, $($t: tt), *) => {
        $(_ = $content.parse::<Token![$t]>();)*
    };
}

#[inline(always)]
pub fn to_boxed_string<T: ToTokens>(x: &T) -> Box::<String> {
    Box::new(x.to_token_stream().to_string().to_lowercase())
}

#[inline(always)]
pub fn to_static_str<T: ToTokens>(x: &T) -> &'static str {
    Box::leak(x.to_token_stream().to_string().to_lowercase().into_boxed_str())
}

pub enum ReturnType {
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

pub fn signature_get_inputs(inputs: Punctuated::<syn::FnArg, Token![,]>) -> Vec::<FnArg> {
    inputs.into_iter().filter_map(|fn_arg| {
        match fn_arg {
            syn::FnArg::Receiver(..) => None,
            syn::FnArg::Typed(PatType { pat, ty, .. }) => {
                let ty = Some(to_boxed_string(&ty));
                if let Pat::Ident(PatIdent { ident, .. }) = *pat {
                    let name = Some(to_boxed_string(&ident));
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
pub fn signature_get_output(output: syn::ReturnType) -> ReturnType {
    match output {
        syn::ReturnType::Default => ReturnType::Default,
        syn::ReturnType::Type(.., ty) => ReturnType::Type(ty),
    }
}

#[inline]
pub fn inputs_to_string(inputs: &Vec::<FnArg>) -> String {
    inputs.iter().map(|FnArg{ty, ..}| quote::quote!(#ty).to_string()).collect()
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

fn parse<'a>(file_path: &'a PathBuf, code: &String) -> syn::Result::<
    (FnSigs::<'a>, StructDefs::<'a>, EnumDefs::<'a>)
> {
    let ast = syn::parse_str::<File>(&code)?;
    let size = ast.items.len() / 2;
    let map = ast.items.into_iter().fold((
        FnSigs::with_capacity(size),
        StructDefs::with_capacity(size),
        EnumDefs::with_capacity(size),
    ), |(mut fnsigs, mut defs, mut edefs), syn_item| {
        let span = syn_item.span();
        match syn_item {
            syn::Item::Fn(f) => {
                let loc = Loc::from_span(file_path, &span);
                let sig = FnSignature::from(f.sig);
                fnsigs.push((loc, sig));
            }
            syn::Item::Struct(s) => {
                let loc = Loc::from_span(file_path, &span);
                let def = StructDef::from(s);
                defs.push((loc, def));
            }
            syn::Item::Enum(e) => {
                let loc = Loc::from_span(file_path, &span);
                let def = EnumDef::from(e);
                edefs.push((loc, def));
            }
            syn::Item::Impl(im) => {
                impl_get_fns(file_path, im).into_iter().for_each(|(sig, loc)| {
                    fnsigs.push((loc, sig));
                })
            }
            _ => {}
        } (fnsigs, defs, edefs)
    });
    Ok(map)
}

pub type Results<'a, 'b> = Vec::<&'a Loc<'b>>;

fn print_results(results: &Results) {
    if results.is_empty() {
        println!("[no results]")
    } else {
        results.par_iter().for_each(|loc| println!("{loc}"))
    }
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

    let (mut edefs_count, mut defs_count) = (0, 0);
    let items = contents.iter().flat_map(|(file_path, code)| {
        if let Ok((fnsigs, defs, edefs)) = parse(file_path, code) {
            defs_count += defs.len();
            edefs_count = edefs.len();
            Some((fnsigs, defs, edefs))
        } else {
            None
        }
    }).collect::<Vec::<_>>();

    let query_item = syn::parse_str::<Item>(&args[1]).unwrap();
    match query_item {
        Item::StructDef(def) => {
            let maps = items.iter().map(|(.., defs, _)| {
                let mut map = StructDefMap::new(defs_count);
                defs.into_iter().for_each(|(loc, def)| map.insert(def, loc));
                map.finalize();
                map
            }).collect::<Vec::<_>>();
            let Some(iter) = def.fields.par_iter() else { return ExitCode::SUCCESS };
            let results = iter.filter_map(|f| {
                if let Some(name) = f.name {
                    Some(maps.iter().flat_map(|map| map.find_names(name, def.is_tup)).collect::<Vec::<_>>())
                } else if let Some(ty) = f.ty {
                    Some(maps.iter().flat_map(|map| map.find_types(ty, def.is_tup)).collect::<Vec::<_>>())
                } else {
                    None
                }
            }).flatten().collect::<Vec::<_>>();
            print_results(&results);
        }
        Item::EnumDef(edef) => {
            let edefs = items.into_iter().flat_map(|(.., edefs)| edefs).collect::<Vec::<_>>();
            print_results(&EnumDef::search_enum_def(&edef, &edefs));
        },
        Item::FnSignature(fnsig) => {
            let maps = items.into_iter().map(|(fnsigs, ..)| {
                fnsigs.into_iter().map(|(loc, fnsig)| (fnsig, loc)).collect::<FnSigMap>()
            }).collect::<Vec::<_>>();
            let results = maps.iter().filter_map(|map| map.get(&fnsig)).collect::<Vec::<_>>();
            print_results(&results);
        }
    };

    println!{
        "[searched in {count} {files}]",
        count = contents.len(),
        files = if contents.len() == 1 { "file" } else { "files" }
    };

    ExitCode::SUCCESS
}

/* TODO:
    (#1) Support searching by lifetimes, generics.
    (#3) Support recursive types for enums
*/
