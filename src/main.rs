use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::fs::read_to_string;
use std::collections::HashMap;

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

mod map;
use map::*;
mod loc;
use loc::*;
mod item;
use item::*;
mod fnsig;
use fnsig::*;
mod fnarg;
use fnarg::*;
mod dir_rec;
use dir_rec::*;
mod structdef;
use structdef::*;

#[macro_export]
macro_rules! skip_tokens {
    ($content: expr, $($t: tt), *) => {
        $(_ = $content.parse::<Token![$t]>();)*
    };
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

    let mut mapp = StructDefStore::new();
    map.values().for_each(|map| {
        map.iter().for_each(|(item, loc)| {
            match item {
                Item::StructDef(ref def) => mapp.insert(def, loc),
                _ => {}
            }
        });
    });

    mapp.finalize();

    let item = syn::parse_str::<Item>(&args[1]).unwrap();
    let results = if let Item::StructDef(ref def) = item {
        def.fields.iter().flat_map(|f| {
            if let Some(ref ident) = f.ident {
                mapp.find_by_field_name(&ident.to_string())
            } else {
                mapp.find_by_field_type(&f.ty.to_token_stream().to_string())
            }
        }).collect()
    } else {
        search_for_item(&item, &map)
    };

    if results.is_empty() {
        println!("[no results]")
    } else {
        results.par_iter().for_each(|loc| println!("{loc}"))
    }

    println!{
        "[searched in {count} {files}]",
        count = contents.len(),
        files = if contents.len() == 1 { "file" } else { "files" }
    };

    ExitCode::SUCCESS
}

/* TODO:
    1. Search not only by types but by names too, but idk how to do that the fastest way
    2. Support lifetimes, generics.
*/
