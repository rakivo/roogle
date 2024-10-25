use std::hash::BuildHasherDefault;
use std::collections::{BTreeSet, HashMap, HashSet};

use quote::ToTokens;
use indexmap::IndexMap;
use twox_hash::XxHash64;
use fst::{Set, SetBuilder, IntoStreamer, automaton::Str};

use crate::loc::Loc;
use crate::structdef::StructDef;

pub struct StructDefStore<'a> {
    by_field_type: IndexMap<String, HashSet<&'a Loc<'a>>, BuildHasherDefault::<XxHash64>>,
    all_defs: HashMap<&'a Loc<'a>, &'a StructDef>,
    field_names: BTreeSet<String>,
    by_field_name: Option<Set<Vec<u8>>>,
}

impl<'a> StructDefStore<'a> {
    pub fn new() -> Self {
        Self {
            by_field_type: IndexMap::with_hasher(BuildHasherDefault::<XxHash64>::default()),
            all_defs: HashMap::new(),
            field_names: BTreeSet::new(),
            by_field_name: None,
        }
    }

    pub fn insert(&mut self, def: &'a StructDef, location: &'a Loc<'a>) {
        def.fields.iter().for_each(|f| {
            if let Some(ident) = &f.ident {
                self.field_names.insert(ident.to_string().to_lowercase());
            }

            let type_str = f.ty.to_token_stream().to_string().to_lowercase();
            self.by_field_type.entry(type_str).or_default().insert(location);
        });
        self.all_defs.insert(location, def);
    }

    pub fn finalize(&mut self) {
        let mut set_builder = SetBuilder::memory();
        for name in self.field_names.iter() {
            set_builder.insert(name).unwrap();
        }
        self.by_field_name = Some(set_builder.into_set());
    }

    pub fn find_by_field_type(&self, field_type: &str) -> Vec<&Loc<'a>> {
        self.by_field_type
            .get(&field_type.to_lowercase())
            .map(|set| set.into_iter().map(std::ops::Deref::deref).collect())
            .unwrap_or_else(Vec::new)
    }

    pub fn find_by_field_name(&self, field_name: &str) -> Vec<&'a Loc<'a>> {
        let mut matches = Vec::new();
        if let Some(ref by_field_name) = self.by_field_name {
            let lower_field_name = field_name.to_lowercase();
            let automaton = Str::new(&lower_field_name);
            let stream = by_field_name.search(automaton);

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
