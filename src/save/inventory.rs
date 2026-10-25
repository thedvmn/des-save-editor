//! The character's inventory in `USER.DAT`.
//!
//! ```text
//! 0x2D4    u32  number of occupied records
//! 0x10360  u32  mirror of the count; the game doesn't always keep it equal
//! 0x2DC    2048 records of 0x20 bytes:
//!   +0x00  u32  category type code (0xFFFFFFFF = empty slot)
//!   +0x04  u32  item ID
//!   +0x08  u32  count
//!   +0x0C  u32  idx1: durability-table key, referenced by the equip pointers
//!   +0x10  u16  sort ID (menu ordering)
//!   +0x12  u16  idx2: display index
//!   +0x14  u32  unknown, 0x01000000 in every observed record
//!   +0x18  8 bytes, zero
//! 0x10364  durability table: 8 bytes per idx1, current durability first
//! ```
//!
//! In every observed save idx1 == idx2 == slot number; new items follow that.

use super::items::{self, ItemCategory, ItemDef};
use super::user_dat::{UserDatError, read_u32, write_u32};

const COUNT: usize = 0x2D4;
const COUNT_MIRROR: usize = 0x10360;
const RECORDS: usize = 0x2DC;
const STRIDE: usize = 0x20;
pub const SLOTS: usize = 0x800;
const DURABILITY: usize = 0x10364;

/// Equip-slot pointers (hands, ammo, armor, rings, quick items). Each holds
/// the idx1 of an inventory record, or 0xFFFFFFFF. 0x260 is unused.
const EQUIP_POINTERS: std::ops::RangeInclusive<usize> = 0x238..=0x27C;
const EQUIP_POINTER_UNUSED: usize = 0x260;

const EMPTY: u32 = 0xFFFF_FFFF;
const DEFAULT_MISC2: u32 = 0x0100_0000;
/// Used when the database has no max durability for a weapon/armor piece.
const FALLBACK_DURABILITY: u32 = 200;

/// An empty record as the game writes it (seen in never-used slots).
const EMPTY_RECORD: [u8; STRIDE] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, //
    0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryItem {
    /// Record slot on disk; `None` for an item added in the editor.
    pub slot: Option<u16>,
    pub category: ItemCategory,
    pub id: u32,
    pub count: u32,
    /// Current durability (weapons and armor only).
    pub durability: u32,
    /// Referenced by an equip slot; must not be removed.
    pub equipped: bool,
    idx1: u32,
    idx2: u16,
    sort_id: u16,
    misc2: u32,
}

impl InventoryItem {
    pub fn def(&self) -> Option<&'static ItemDef> {
        items::lookup(self.category, self.id)
    }

    pub fn name(&self) -> String {
        match self.def() {
            Some(d) => d.name.to_owned(),
            None => format!("Unknown item {:#x}", self.id),
        }
    }

    /// Menu sort ID; reused when the same item is added to the stockpile.
    pub fn sort_id(&self) -> u16 {
        self.sort_id
    }
}

fn record(slot: usize) -> usize {
    RECORDS + slot * STRIDE
}

fn durability_offset(idx1: u32) -> Option<usize> {
    ((idx1 as usize) < SLOTS).then(|| DURABILITY + idx1 as usize * 8)
}

fn read_u16(b: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([b[off], b[off + 1]])
}

fn write_u16(b: &mut [u8], off: usize, v: u16) {
    b[off..off + 2].copy_from_slice(&v.to_be_bytes());
}

fn equipped_idx1s(bytes: &[u8]) -> Vec<u32> {
    EQUIP_POINTERS
        .step_by(4)
        .filter(|&o| o != EQUIP_POINTER_UNUSED)
        .map(|o| read_u32(bytes, o))
        .filter(|&p| p != EMPTY)
        .collect()
}

fn occupied_slots(bytes: &[u8]) -> impl Iterator<Item = usize> + '_ {
    (0..SLOTS).filter(|&s| read_u32(bytes, record(s)) != EMPTY)
}

pub fn read(bytes: &[u8]) -> Result<Vec<InventoryItem>, UserDatError> {
    let equipped = equipped_idx1s(bytes);
    let mut items = Vec::new();

    for slot in occupied_slots(bytes) {
        let r = record(slot);
        let code = read_u32(bytes, r);
        let category = ItemCategory::from_type_code(code).ok_or_else(|| {
            UserDatError::CorruptInventory(format!("unknown item type {code:#x} in slot {slot}"))
        })?;
        let idx1 = read_u32(bytes, r + 0x0C);
        let durability = match durability_offset(idx1) {
            Some(o) if category.has_durability() => read_u32(bytes, o),
            Some(_) => 0,
            None => {
                return Err(UserDatError::CorruptInventory(format!(
                    "bad index {idx1:#x} in slot {slot}"
                )));
            }
        };

        items.push(InventoryItem {
            slot: Some(slot as u16),
            category,
            id: read_u32(bytes, r + 0x04),
            count: read_u32(bytes, r + 0x08),
            durability,
            equipped: equipped.contains(&idx1),
            idx1,
            sort_id: read_u16(bytes, r + 0x10),
            idx2: read_u16(bytes, r + 0x12),
            misc2: read_u32(bytes, r + 0x14),
        });
    }

    let stored = read_u32(bytes, COUNT) as usize;
    if stored != items.len() {
        return Err(UserDatError::CorruptInventory(format!(
            "item count is {stored} but {} slots are occupied",
            items.len()
        )));
    }
    Ok(items)
}

/// Writes `items` in place: existing items are updated in their own slots,
/// removed ones are cleared and new ones go into the first free slots.
pub fn write(items: &[InventoryItem], bytes: &mut [u8]) -> Result<(), UserDatError> {
    let occupied_before = occupied_slots(bytes).count() as u32;
    let kept: Vec<usize> = items
        .iter()
        .filter_map(|i| i.slot.map(usize::from))
        .collect();
    let removed: Vec<usize> = occupied_slots(bytes)
        .filter(|s| !kept.contains(s))
        .collect();

    // Removing an equipped item would leave a dangling equip pointer.
    let equipped = equipped_idx1s(bytes);
    if let Some(&slot) = removed
        .iter()
        .find(|&&s| equipped.contains(&read_u32(bytes, record(s) + 0x0C)))
    {
        return Err(UserDatError::RemovedEquipped(slot));
    }

    for &slot in &removed {
        let r = record(slot);
        if let Some(o) = durability_offset(read_u32(bytes, r + 0x0C)) {
            write_u32(bytes, o, EMPTY);
        }
        bytes[r..r + STRIDE].copy_from_slice(&EMPTY_RECORD);
    }

    let free: Vec<usize> = (0..SLOTS)
        .filter(|&s| read_u32(bytes, record(s)) == EMPTY)
        .collect();
    let mut free = free.into_iter();
    for item in items {
        let (slot, idx1, idx2) = match item.slot {
            Some(s) => (usize::from(s), item.idx1, item.idx2),
            None => {
                let s = free.next().ok_or(UserDatError::InventoryFull)?;
                bytes[record(s)..record(s) + STRIDE].fill(0);
                (s, s as u32, s as u16)
            }
        };

        let r = record(slot);
        write_u32(bytes, r, item.category.type_code());
        write_u32(bytes, r + 0x04, item.id);
        write_u32(bytes, r + 0x08, item.count);
        write_u32(bytes, r + 0x0C, idx1);
        write_u16(bytes, r + 0x10, item.sort_id);
        write_u16(bytes, r + 0x12, idx2);
        write_u32(bytes, r + 0x14, item.misc2);
        if item.category.has_durability() {
            if let Some(o) = durability_offset(idx1) {
                write_u32(bytes, o, item.durability);
            }
        }
    }

    write_u32(bytes, COUNT, items.len() as u32);
    // Shift the mirror by the change rather than overwriting it, since saves
    // from the game can have it off by one.
    let mirror =
        (read_u32(bytes, COUNT_MIRROR) + items.len() as u32).saturating_sub(occupied_before);
    write_u32(bytes, COUNT_MIRROR, mirror);
    Ok(())
}

/// Adds `count` of `def`. Stackable items are merged into an existing stack
/// (up to the stack limit); everything else gets a new record.
pub fn add(items: &mut Vec<InventoryItem>, def: &ItemDef, count: u32) {
    let count = count.clamp(1, def.max_count());
    if def.is_stackable() {
        if let Some(existing) = items
            .iter_mut()
            .find(|i| i.category == def.category && i.id == def.id)
        {
            existing.count = (existing.count + count).min(def.max_count().max(existing.count));
            return;
        }
    }

    // Copy the sort ID from an identical item if there is one; otherwise 0,
    // which only affects where the item appears in the in-game menu.
    let sort_id = items
        .iter()
        .find(|i| i.category == def.category && i.id == def.id)
        .map_or(0, |i| i.sort_id);

    items.push(InventoryItem {
        slot: None,
        category: def.category,
        id: def.id,
        count,
        durability: if def.category.has_durability() {
            def.max_durability.unwrap_or(FALLBACK_DURABILITY)
        } else {
            0
        },
        equipped: false,
        idx1: 0,
        idx2: 0,
        sort_id,
        misc2: DEFAULT_MISC2,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::user_dat::MIN_SIZE;

    fn empty_inventory() -> Vec<u8> {
        let mut b = vec![0u8; MIN_SIZE];
        for s in 0..SLOTS {
            b[record(s)..record(s) + STRIDE].copy_from_slice(&EMPTY_RECORD);
        }
        for o in EQUIP_POINTERS.step_by(4) {
            write_u32(&mut b, o, EMPTY);
        }
        b
    }

    fn def(cat: ItemCategory, id: u32) -> &'static ItemDef {
        items::lookup(cat, id).unwrap()
    }

    #[test]
    fn add_write_read_remove() {
        let mut b = empty_inventory();
        let mut inv = read(&b).unwrap();
        assert!(inv.is_empty());

        add(&mut inv, def(ItemCategory::Weapon, 0x2710), 5); // Dagger: count clamps to 1
        add(&mut inv, def(ItemCategory::Goods, 0x3E8), 10); // Crescent Moon Grass
        add(&mut inv, def(ItemCategory::Goods, 0x3E8), 95); // stacks, capped at 99
        assert_eq!(inv.len(), 2);
        write(&inv, &mut b).unwrap();

        let back = read(&b).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(
            (back[0].slot, back[0].id, back[0].count, back[0].durability),
            (Some(0), 0x2710, 1, 200)
        );
        assert_eq!((back[1].slot, back[1].count), (Some(1), 99));
        assert_eq!(read_u32(&b, record(1) + 0x0C), 1, "idx1 == slot");
        assert_eq!(read_u32(&b, COUNT_MIRROR), 2);

        // Removing the dagger restores the game's empty-slot pattern.
        let remaining = vec![back[1].clone()];
        write(&remaining, &mut b).unwrap();
        assert_eq!(&b[record(0)..record(0) + STRIDE], &EMPTY_RECORD);
        assert_eq!(read_u32(&b, DURABILITY), EMPTY);
        assert_eq!(read(&b).unwrap(), remaining);
    }

    #[test]
    fn refuses_to_remove_equipped() {
        let mut b = empty_inventory();
        let mut inv = Vec::new();
        add(&mut inv, def(ItemCategory::Weapon, 0x2710), 1);
        write(&inv, &mut b).unwrap();
        write_u32(&mut b, 0x23C, 0); // equip slot 0 in the right hand

        assert!(read(&b).unwrap()[0].equipped);
        assert_eq!(write(&[], &mut b), Err(UserDatError::RemovedEquipped(0)));
    }

    #[test]
    fn detects_count_mismatch() {
        let mut b = empty_inventory();
        write_u32(&mut b, COUNT, 3);
        assert!(matches!(read(&b), Err(UserDatError::CorruptInventory(_))));
    }
}
