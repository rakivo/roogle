use std::ops::Deref;
use std::hash::BuildHasherDefault;
use std::collections::{BTreeSet, HashMap, HashSet};

use quote::ToTokens;
use indexmap::IndexMap;
use twox_hash::XxHash64;
use fst::{Set, SetBuilder, IntoStreamer, automaton::Str};

use crate::loc::Loc;
use crate::structdef::StructDef;

pub type Names<'a> = Set::<Vec::<u8>>;

pub type Types<'a> = IndexMap::<
    String,
    HashSet::<&'a Loc<'a>>,
    BuildHasherDefault::<XxHash64>
>;

pub type Xx64Hasher = BuildHasherDefault::<XxHash64>;

pub struct StructDefMap<'a> {
    types: Types<'a>,
    names: Option::<Names<'a>>,
    field_names: BTreeSet::<String>,
    all_defs: HashMap::<&'a Loc::<'a>, &'a StructDef>
}

impl<'a> StructDefMap<'a> {
    pub fn new(defs_len: usize) -> Self {
        Self {
            types: Types::with_capacity_and_hasher(defs_len * 2, Xx64Hasher::default()),
            names: None,
            all_defs: HashMap::with_capacity(defs_len),
            field_names: BTreeSet::new()
        }
    }

    pub fn insert(&mut self, def: &'a StructDef, location: &'a Loc<'a>) {
        def.fields.iter().for_each(|f| {
            if let Some(ref ident) = f.ident {
                self.field_names.insert(ident.to_string().to_lowercase());
            }
            let type_str = f.ty.to_token_stream().to_string().to_lowercase();
            self.types.entry(type_str).or_default().insert(location);
        });
        self.all_defs.insert(location, def);
    }

    pub fn finalize(&mut self) {
        let mut set_builder = SetBuilder::memory();
        self.field_names.iter().for_each(|name| {
            set_builder.insert(name).unwrap();
        });
        self.names = Some(set_builder.into_set());
    }

    pub fn find_types(&self, field_type: &str) -> Vec<&Loc<'a>> {
        self.types.get(&field_type.to_lowercase())
            .map(|set| set.into_iter().map(Deref::deref).collect())
            .unwrap_or_else(Vec::new)
    }

    pub fn find_names(&self, field_name: &str) -> Vec<&'a Loc<'a>> {
        let mut matches = Vec::new();
        if let Some(ref names) = self.names {
            let lower_field_name = field_name.to_lowercase();
            let automaton = Str::new(&lower_field_name);
            let stream = names.search(automaton);

            let found_names = stream.into_stream().into_strs().unwrap_or_default();

            for name in found_names {
                matches.extend(
                    self.all_defs.iter().filter_map(|(loc, def)| {
                        if def.fields.iter().any(|f| f.ident.as_ref().map_or(false, |i| i.to_string().to_lowercase() == name)) {
                            Some(loc)
                        } else {
                            None
                        }
                    }),
                );
            }
        }
        matches
    }
}
