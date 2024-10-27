use std::collections::{HashMap, HashSet};
use std::hash::BuildHasherDefault;
use rayon::prelude::*;
use twox_hash::XxHash64;

use crate::loc::Loc;
use crate::fields::Fields;
use crate::enumdef::EnumDef;

pub struct EnumDefMap<'a> {
    types: HashMap::<&'static str, HashSet::<&'a Loc<'a>>, BuildHasherDefault::<XxHash64>>,
    names: HashMap::<&'static str, HashSet::<&'a Loc<'a>>, BuildHasherDefault::<XxHash64>>,
    variant_defs: HashMap::<&'a Loc::<'a>, &'a EnumDef>,
}

impl<'a> EnumDefMap<'a> {
    pub fn new(defs_len: usize) -> Self {
        Self {
            types: HashMap::with_capacity_and_hasher(defs_len * 2, BuildHasherDefault::<XxHash64>::default()),
            names: HashMap::with_capacity_and_hasher(defs_len * 2, BuildHasherDefault::<XxHash64>::default()),
            variant_defs: HashMap::with_capacity(defs_len),
        }
    }

    pub fn insert(&mut self, def: &'a EnumDef, loc: &'a Loc::<'a>) {
        def.variants.iter().for_each(|variant| {
            if let Some(name) = variant.name {
                self.names.entry(name).or_default().insert(loc);
            }
            self.insert_variant_types(&variant.fields, loc);
        });
        self.variant_defs.insert(loc, def);
    }

    fn insert_variant_types(&mut self, fields: &Fields, loc: &'a Loc::<'a>) {
        fields.iter().for_each(|f| {
            if let Some(name) = f.name {
                self.names.entry(name).or_default().insert(loc);
            }
            if let Some(ty) = f.ty {
                self.types.entry(ty).or_default().insert(loc);
            }
        });
    }

    pub fn find_types(&self, field_type: &str) -> Vec::<&Loc::<'a>> {
        self.types.get(field_type)
            .map(|locs| locs.par_iter().map(std::ops::Deref::deref).collect())
            .unwrap_or_default()
    }

    pub fn find_names(&self, field_name: &str) -> Vec::<&Loc::<'a>> {
        self.names.get(field_name)
            .map(|locs| locs.par_iter().map(std::ops::Deref::deref).collect())
            .unwrap_or_default()
    }
}
