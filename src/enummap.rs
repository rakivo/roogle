use twox_hash::XxHash64;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

use crate::Results;
use crate::enumdef::*;

type FastHashMap<K, V> = HashMap<K, V, BuildHasherDefault::<XxHash64>>;

pub struct EnumMap<'a> {
    pub name_map: FastHashMap::<&'static str, Results<'a, 'a>>,
    pub type_map: FastHashMap::<&'static str, Results<'a, 'a>>,
}

impl<'a> EnumMap<'a> {
    pub fn new(enums: &'a EnumDefs) -> Self {
        let mut name_map = FastHashMap::<&'static str, Results<'a, 'a>>::default();
        let mut type_map = FastHashMap::<&'static str, Results<'a, 'a>>::default();

        enums.iter().for_each(|(loc, enum_def)| {
            if let Some(name) = &enum_def.name {
                name_map.entry(name).or_default().push(loc);
            }
            for variant in &enum_def.variants {
                if let Some(name) = &variant.name {
                    name_map.entry(name).or_default().push(loc);
                }
                for f in variant.fields.iter() {
                    if let Some(name) = &f.name {
                        name_map.entry(name).or_default().push(loc);
                    }
                    if let Some(ty) = &f.ty {
                        type_map.entry(ty).or_default().push(loc);
                    }
                }
            }
        });

        Self { name_map, type_map }
    }
}
