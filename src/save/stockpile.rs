//! Items stored with Stockpile Thomas in the Nexus.
//!
//! ```text
//! 0x14BE8  2048 entries of 0x14 bytes:
//!   +0x00  u32  unknown (usually 0; preserved)
//!   +0x04  u8   category (0x00 weapon, 0x10 armor, 0x20 ring, 0x40 goods, 0xFF empty)
//!   +0x05  u24  item ID
//!   +0x08  u32  hi16 = sort ID (same as the inventory's), lo16 = order index
//!   +0x0C  u8   count, low 8 bits
//!   +0x0D  u8   bits 6-7 = count bits 8-9, bits 0-5 = flag (0x21 when occupied)
//!   +0x0E  4 bytes, usually zero (preserved)
//!   +0x12  u16  durability
//! 0x1EBEC  u32  item count, roughly the number of occupied entries
//! ```
//!
//! The game leaves gaps between entries, so like the inventory this is
//! edited in place rather than rebuilt.
//!
//! The count at 0x1EBEC is not exact: saves written by the game can have it
//! above the number of occupied entries (e.g. 109 vs 105), so it isn't used
//! for validation and edits only shift it by the change in entries.

use super::items::{self, ItemCategory, ItemDef};
use super::user_dat::{UserDatError, read_u32, write_u32};

const BASE: usize = 0x14BE8;
const STRIDE: usize = 0x14;
pub const SLOTS: usize = 2048;
const COUNT: usize = 0x1EBEC;

const EMPTY_TYPE: u8 = 0xFF;
const OCCUPIED_FLAG: u8 = 0x21;
/// Largest count the 10-bit field can hold.
pub const MAX_COUNT: u32 = 0x3FF;
const FALLBACK_DURABILITY: u32 = 200;

/// An empty entry as the game writes it.
const EMPTY_ENTRY: [u8; STRIDE] = [
    0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF,
];

#[derive(Debug, Clone, PartialEq)]
pub struct StockpileItem {
    /// Entry slot on disk; `None` for an item added in the editor.
    pub slot: Option<u16>,
    pub category: ItemCategory,
    pub id: u32,
    pub count: u32,
    pub durability: u32,
    unknown: u32,
    sort_order: u32,
}

impl StockpileItem {
    pub fn def(&self) -> Option<&'static ItemDef> {
        items::lookup(self.category, self.id)
    }

    pub fn name(&self) -> String {
        match self.def() {
            Some(d) => d.name.to_owned(),
            None => format!("Unknown item {:#x}", self.id),
        }
    }
}

fn entry(slot: usize) -> usize {
    BASE + slot * STRIDE
}

fn type_byte(category: ItemCategory) -> u8 {
    (category.type_code() >> 24) as u8
}

fn occupied_slots(bytes: &[u8]) -> impl Iterator<Item = usize> + '_ {
    (0..SLOTS).filter(|&s| bytes[entry(s) + 4] != EMPTY_TYPE)
}

pub fn read(bytes: &[u8]) -> Result<Vec<StockpileItem>, UserDatError> {
    let mut items = Vec::new();
    for slot in occupied_slots(bytes) {
        let e = entry(slot);
        let category =
            ItemCategory::from_type_code(u32::from(bytes[e + 4]) << 24).ok_or_else(|| {
                UserDatError::CorruptStockpile(format!(
                    "unknown item type {:#x} in slot {slot}",
                    bytes[e + 4]
                ))
            })?;
        items.push(StockpileItem {
            slot: Some(slot as u16),
            category,
            id: read_u32(bytes, e + 4) & 0x00FF_FFFF,
            count: (u32::from(bytes[e + 0x0D] >> 6) << 8) | u32::from(bytes[e + 0x0C]),
            durability: u32::from(u16::from_be_bytes([bytes[e + 0x12], bytes[e + 0x13]])),
            unknown: read_u32(bytes, e),
            sort_order: read_u32(bytes, e + 8),
        });
    }

    Ok(items)
}

fn write_entry(bytes: &mut [u8], slot: usize, item: &StockpileItem, sort_order: u32) {
    let e = entry(slot);
    let count = item.count.min(MAX_COUNT);
    write_u32(bytes, e, item.unknown);
    write_u32(
        bytes,
        e + 4,
        (u32::from(type_byte(item.category)) << 24) | (item.id & 0x00FF_FFFF),
    );
    write_u32(bytes, e + 8, sort_order);
    bytes[e + 0x0C] = count as u8;
    bytes[e + 0x0D] = ((count >> 8) as u8) << 6 | OCCUPIED_FLAG;
    bytes[e + 0x12..e + 0x14].copy_from_slice(&(item.durability.min(0xFFFF) as u16).to_be_bytes());
}

/// Writes `items` in place: existing entries are updated in their own slots,
/// removed ones are cleared and new ones go into the first free slots.
pub fn write(items: &[StockpileItem], bytes: &mut [u8]) -> Result<(), UserDatError> {
    let occupied_before = occupied_slots(bytes).count() as u32;
    let kept: Vec<usize> = items
        .iter()
        .filter_map(|i| i.slot.map(usize::from))
        .collect();
    let removed: Vec<usize> = occupied_slots(bytes)
        .filter(|s| !kept.contains(s))
        .collect();
    for slot in removed {
        bytes[entry(slot)..entry(slot) + STRIDE].copy_from_slice(&EMPTY_ENTRY);
    }

    let free: Vec<usize> = (0..SLOTS)
        .filter(|&s| bytes[entry(s) + 4] == EMPTY_TYPE)
        .collect();
    let mut free = free.into_iter();
    for item in items {
        match item.slot {
            Some(s) => write_entry(bytes, usize::from(s), item, item.sort_order),
            None => {
                let s = free.next().ok_or(UserDatError::StockpileFull)?;
                bytes[entry(s) + 0x0E..entry(s) + 0x12].fill(0);
                // New entries use their slot as the order index.
                write_entry(bytes, s, item, (item.sort_order & 0xFFFF_0000) | s as u32);
            }
        }
    }

    let count = (read_u32(bytes, COUNT) + items.len() as u32).saturating_sub(occupied_before);
    write_u32(bytes, COUNT, count);
    Ok(())
}

/// Adds `count` of `def`, merging stackable items into an existing entry.
/// `sort_id` should come from an identical item elsewhere if one exists.
pub fn add(items: &mut Vec<StockpileItem>, def: &ItemDef, count: u32, sort_id: Option<u16>) {
    let max = def.max_count().min(MAX_COUNT);
    let count = count.clamp(1, max);
    if def.is_stackable() {
        if let Some(existing) = items
            .iter_mut()
            .find(|i| i.category == def.category && i.id == def.id)
        {
            existing.count = (existing.count + count).min(max.max(existing.count));
            return;
        }
    }

    let sort_id = sort_id
        .or_else(|| {
            let same = items
                .iter()
                .find(|i| i.category == def.category && i.id == def.id)?;
            Some((same.sort_order >> 16) as u16)
        })
        .unwrap_or(0);

    items.push(StockpileItem {
        slot: None,
        category: def.category,
        id: def.id,
        count,
        durability: match def.category {
            ItemCategory::Weapon | ItemCategory::Armor => {
                def.max_durability.unwrap_or(FALLBACK_DURABILITY)
            }
            _ => 0,
        },
        unknown: 0,
        sort_order: u32::from(sort_id) << 16,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::user_dat::MIN_SIZE;

    fn empty_stockpile() -> Vec<u8> {
        let mut b = vec![0u8; MIN_SIZE];
        for s in 0..SLOTS {
            b[entry(s)..entry(s) + STRIDE].copy_from_slice(&EMPTY_ENTRY);
        }
        b
    }

    #[test]
    fn add_write_read_remove() {
        let mut b = empty_stockpile();
        let mut pile = read(&b).unwrap();
        let grass = items::lookup(ItemCategory::Goods, 0x3E8).unwrap();
        let heavy_arrow = items::lookup(ItemCategory::Weapon, 0x27164).unwrap();
        let dagger = items::lookup(ItemCategory::Weapon, 0x2710).unwrap();

        add(&mut pile, dagger, 1, None);
        add(&mut pile, heavy_arrow, 300, Some(0x0517)); // count > 255 needs the high bits
        add(&mut pile, grass, 5, None);
        write(&pile, &mut b).unwrap();

        let back = read(&b).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!((back[0].id, back[0].durability), (0x2710, 200));
        assert_eq!((back[1].count, back[1].sort_order >> 16), (300, 0x0517));
        assert_eq!(b[entry(1) + 0x0D], (1 << 6) | OCCUPIED_FLAG);
        assert_eq!(back[2].count, 5);

        // Removing the dagger leaves a gap, as the game does.
        let rest = back[1..].to_vec();
        write(&rest, &mut b).unwrap();
        assert_eq!(&b[entry(0)..entry(0) + STRIDE], &EMPTY_ENTRY);
        assert_eq!(read(&b).unwrap(), rest);
        assert_eq!(read_u32(&b, COUNT), 2);
    }

    #[test]
    fn inexact_count_is_kept_and_shifted() {
        let mut b = empty_stockpile();
        let mut pile = Vec::new();
        add(
            &mut pile,
            items::lookup(ItemCategory::Weapon, 0x2710).unwrap(),
            1,
            None,
        );
        write(&pile, &mut b).unwrap();
        // The game can leave the count above the number of occupied entries.
        write_u32(&mut b, COUNT, 5);

        let mut pile = read(&b).unwrap();
        assert_eq!(pile.len(), 1);
        let before = b.clone();
        write(&pile, &mut b).unwrap();
        assert_eq!(b, before, "no-op write keeps the stored count");

        add(
            &mut pile,
            items::lookup(ItemCategory::Goods, 0x3E8).unwrap(),
            1,
            None,
        );
        write(&pile, &mut b).unwrap();
        assert_eq!(read_u32(&b, COUNT), 6);
        write(&pile[1..], &mut b).unwrap();
        assert_eq!(read_u32(&b, COUNT), 5);
    }
}
