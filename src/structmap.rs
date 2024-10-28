use std::hash::BuildHasherDefault;
use std::collections::{BTreeSet, HashMap, HashSet};

use rayon::prelude::*;
use indexmap::IndexMap;
use twox_hash::XxHash64;
use fst::{Set, SetBuilder, IntoStreamer, automaton::Str};

use crate::Results;
use crate::loc::Loc;
use crate::structdef::StructDef;

pub type Names<'a> = Set::<Vec::<u8>>;

pub type Types<'a> = IndexMap::<
    &'static str,
    HashSet::<&'a Loc<'a>>,
    BuildHasherDefault::<XxHash64>
>;

pub type Xx64Hasher = BuildHasherDefault::<XxHash64>;

pub struct StructDefMap<'a> {
    types: Types<'a>,
    names: Option::<Names<'a>>,
    field_names: BTreeSet::<&'static str>,
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

    pub fn insert(&mut self, def: &'a StructDef, loc: &'a Loc<'a>) {
        def.fields.iter().for_each(|f| {
            if let Some(name) = f.name {
                self.field_names.insert(name);
            }
            if let Some(ty) = f.ty {
                self.types.entry(ty).or_default().insert(loc);
            }
        });
        self.all_defs.insert(loc, def);
    }

    #[inline]
    pub fn finalize(&mut self) {
        let mut set_builder = SetBuilder::memory();
        self.field_names.iter().for_each(|name| unsafe {
            set_builder.insert(name).unwrap_unchecked();
        });
        self.names = Some(set_builder.into_set());
    }

    #[inline]
    pub fn find_types(&self, field_type: &str, is_tup: bool) -> Results {
        self.types.get(field_type).map(|set| {
            set.par_iter()
                .filter(|loc| matches!(self.all_defs.get(*loc), Some(def) if def.is_tup == is_tup))
                .map(std::ops::Deref::deref)
                .collect()
        }).unwrap_or_else(Vec::new)
    }

    pub fn find_names(&self, field_name: &str, is_tup: bool) -> Results {
        let mut matches = Vec::new();
        let Some(ref names) = self.names else { return matches };
        let automaton = Str::new(field_name);
        let stream = names.search(automaton);
        let Ok(names) = stream.into_stream().into_strs() else { return matches };
        for name in names {
            matches.par_extend(
                self.all_defs.par_iter().filter_map(|(loc, def)| {
                    if def.is_tup != is_tup { return None }
                    let Some(iter) = def.fields.par_iter() else { return None };
                    if iter.any(|f| f.name.map_or(false, |i| i == name)) {
                        Some(loc)
                    } else {
                        None
                    }
                })
            );
        } matches
    }
}
