//! Item database: names and properties for every known item ID.
//!
//! Embedded from `data/items.tsv` (see the header of that file for its source).

use std::collections::HashMap;
use std::sync::LazyLock;

/// Inventory category. The on-disk type code is the record's first u32.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemCategory {
    /// Weapons, shields, bows, ammo and catalysts/talismans.
    Weapon,
    Armor,
    Ring,
    Goods,
}

impl ItemCategory {
    pub const ALL: [ItemCategory; 4] = [Self::Weapon, Self::Armor, Self::Ring, Self::Goods];

    pub fn label(self) -> &'static str {
        match self {
            Self::Weapon => "Weapons",
            Self::Armor => "Armor",
            Self::Ring => "Rings",
            Self::Goods => "Goods",
        }
    }

    pub fn type_code(self) -> u32 {
        match self {
            Self::Weapon => 0x0000_0000,
            Self::Armor => 0x1000_0000,
            Self::Ring => 0x2000_0000,
            Self::Goods => 0x4000_0000,
        }
    }

    pub fn from_type_code(code: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.type_code() == code)
    }

    /// Weapons and armor have an entry in the durability table.
    pub fn has_durability(self) -> bool {
        matches!(self, Self::Weapon | Self::Armor)
    }

    fn from_tsv(s: &str) -> Option<Self> {
        match s {
            "weapons" => Some(Self::Weapon),
            "armor" => Some(Self::Armor),
            "rings" => Some(Self::Ring),
            "goods" => Some(Self::Goods),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ItemDef {
    pub category: ItemCategory,
    pub id: u32,
    pub name: &'static str,
    /// Sub-type such as "Straight Sword", "Head" or "Key".
    pub group: &'static str,
    pub max_durability: Option<u32>,
    /// Experimental / leftover entries not obtainable in normal play.
    pub hidden: bool,
    /// Lower-cased name, for searching.
    pub search_key: String,
}

impl ItemDef {
    pub fn is_ammo(&self) -> bool {
        self.category == ItemCategory::Weapon && matches!(self.group, "Arrow" | "Bolt")
    }

    /// Several items can share one inventory record (goods and ammo).
    pub fn is_stackable(&self) -> bool {
        self.category == ItemCategory::Goods || self.is_ammo()
    }

    pub fn max_count(&self) -> u32 {
        if self.is_ammo() {
            999
        } else if self.is_stackable() {
            99
        } else {
            1
        }
    }

    /// The game's "nothing equipped" stand-ins (Bare Fists, Bare Head, ...).
    pub fn is_placeholder(&self) -> bool {
        self.name.starts_with("Bare ")
    }
}

const TSV: &str = include_str!("../../data/items.tsv");

static ITEMS: LazyLock<Vec<ItemDef>> = LazyLock::new(|| {
    TSV.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let f: Vec<&'static str> = line.split('\t').collect();
            assert_eq!(f.len(), 6, "bad items.tsv line: {line}");
            ItemDef {
                category: ItemCategory::from_tsv(f[0])
                    .unwrap_or_else(|| panic!("bad category: {line}")),
                id: u32::from_str_radix(f[1].trim_start_matches("0x"), 16)
                    .unwrap_or_else(|_| panic!("bad id: {line}")),
                name: f[2],
                group: f[3],
                max_durability: f[4].parse().ok(),
                hidden: f[5] == "1",
                search_key: f[2].to_lowercase(),
            }
        })
        .collect()
});

static INDEX: LazyLock<HashMap<(ItemCategory, u32), usize>> = LazyLock::new(|| {
    ITEMS
        .iter()
        .enumerate()
        .map(|(i, d)| ((d.category, d.id), i))
        .collect()
});

pub fn all() -> &'static [ItemDef] {
    &ITEMS
}

pub fn lookup(category: ItemCategory, id: u32) -> Option<&'static ItemDef> {
    INDEX.get(&(category, id)).map(|&i| &ITEMS[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_loads() {
        assert!(all().len() > 2000);
        let dagger = lookup(ItemCategory::Weapon, 0x2710).unwrap();
        assert_eq!(dagger.name, "Dagger");
        assert_eq!(dagger.max_count(), 1);
        // IDs are only unique within a category.
        assert_eq!(
            lookup(ItemCategory::Armor, 0x18768).unwrap().name,
            "Assassin's Mask"
        );
        assert_eq!(
            lookup(ItemCategory::Goods, 0x3E8).unwrap().name,
            "Crescent Moon Grass"
        );
    }
}
