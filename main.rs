use std::env;
use std::process::ExitCode;
use std::fs::read_to_string;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};

use syn::spanned::Spanned;
use syn::{File, Item as SynItem, Result as SynResult, parse_str};

//            file_path, line, column
struct Loc<'a>(&'a str, usize, usize);

impl Display for Loc<'_> {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{f}:{l}:{c}", f = self.0, l = self.1, c = self.2)
    }
}

struct Item<'a> {
    item: SynItem,
    loc: Loc::<'a>
}

impl Debug for Item<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{loc}", loc = self.loc)
    }
}

type FileMap<'a> = HashMap::<&'a String, ItemMap<'a>>;
type ItemMap<'a> = HashMap::<String, Item::<'a>>;

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

fn parse<'a>(code: String, file_path: &'a str) -> SynResult::<ItemMap::<'a>> {
    let ast = parse_str::<File>(&code)?;
    let map = ast.items.into_iter().filter_map(|syn_item| {
        let linecol = syn_item.span().start();
        let loc = Loc(file_path, linecol.line, linecol.column);
        if let Some(ident) = get_ident(&syn_item) {
            let item = Item { item: syn_item, loc };
            Some((ident, item))
        } else {
            None
        }
    }).collect();
    Ok(map)
}

fn main() -> ExitCode {
    let args = env::args().collect::<Vec::<_>>();
    if args.len() < 2 {
        eprintln!("usage: <{program}> <files_to_index...>", program = args[0]);
        return ExitCode::FAILURE
    }

    let ref file_paths = args[1..];
    let map = file_paths.iter().filter_map(|file_path| {
        read_to_string(file_path).ok().map(|code| (file_path, code))
    }).filter_map(|(file_path, code)| {
        parse(code, file_path).ok().map(|map| (file_path, map))
    }).collect::<FileMap>();

    ExitCode::SUCCESS
}
