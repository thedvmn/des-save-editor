//! Reader/writer for the character data in a Demon's Souls `USER.DAT` file.
//!
//! The file is an unencrypted (on RPCS3), big-endian blob with no checksum.
//! Offsets are taken from Wulf2k/DeS-SaveEdit and jij3x/DemonSave-PS3, which
//! agree on every field used here.
//!
//! Writing is done in place over the original bytes so that every field we
//! don't understand is preserved untouched.

use std::fmt;

use super::inventory::{self, InventoryItem};
use super::stockpile::{self, StockpileItem};

/// Smallest buffer that covers every fixed offset used by the game save.
pub const MIN_SIZE: usize = 0x22000;

mod off {
    pub const SOULS: usize = 0xBC;
    pub const SOUL_MEMORY: usize = 0xC8;
    pub const LEVELS_PURCHASED: usize = 0xCC;
    /// UTF-16BE, zero-terminated, up to 16 code units.
    pub const NAME: usize = 0xD4;
    pub const NAME_MAX_UNITS: usize = 16;
    /// Zeroed-out (deleted / never used) slot files have 0 here.
    pub const SANITY_CHECK: usize = 0x170;

    /// Each stat is stored as a (base, effective) pair of u32s; base comes first.
    /// Order: VIT, INT, END, STR, DEX, MAG, FTH, LCK.
    pub const STATS_BASE: usize = 0x7C;
    pub const STAT_PAIR_STRIDE: usize = 8;

    pub const CHAR_TENDENCY: usize = 0x1EBF0;
    /// World tendencies are each written twice: (value, mirror).
    pub const NEXUS_TENDENCY: usize = 0x1EBF8;
    pub const W1_TENDENCY: usize = 0x1EC00; // Boletarian Palace
    pub const W4_TENDENCY: usize = 0x1EC08; // Shrine of Storms
    pub const W3_TENDENCY: usize = 0x1EC10; // Tower of Latria
    pub const W5_TENDENCY: usize = 0x1EC18; // Valley of Defilement
    pub const W2_TENDENCY: usize = 0x1EC20; // Stonefang Tunnel
    pub const MIRROR: usize = 4;

    /// New Game+ cycle counter (single byte).
    pub const CLEAR_COUNT: usize = 0x1EC58;
}

#[derive(Debug, PartialEq, Eq)]
pub enum UserDatError {
    TooSmall(usize),
    EmptySlot,
    CorruptInventory(String),
    InventoryFull,
    CorruptStockpile(String),
    StockpileFull,
    /// Inventory slot of an equipped item that was removed.
    RemovedEquipped(usize),
}

impl fmt::Display for UserDatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall(len) => write!(
                f,
                "file is too small to be a Demon's Souls save ({len} bytes, need at least {MIN_SIZE})"
            ),
            Self::EmptySlot => write!(f, "save slot is empty"),
            Self::CorruptInventory(msg) => write!(f, "inventory looks corrupt: {msg}"),
            Self::InventoryFull => write!(f, "inventory is full"),
            Self::CorruptStockpile(msg) => write!(f, "Thomas's stockpile looks corrupt: {msg}"),
            Self::StockpileFull => write!(f, "Thomas's stockpile is full"),
            Self::RemovedEquipped(slot) => {
                write!(
                    f,
                    "the item in inventory slot {slot} is equipped and can't be removed"
                )
            }
        }
    }
}

impl std::error::Error for UserDatError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    pub vitality: u32,
    pub intelligence: u32,
    pub endurance: u32,
    pub strength: u32,
    pub dexterity: u32,
    pub magic: u32,
    pub faith: u32,
    pub luck: u32,
}

impl Stats {
    pub const NAMES: [&'static str; 8] = [
        "Vitality",
        "Intelligence",
        "Endurance",
        "Strength",
        "Dexterity",
        "Magic",
        "Faith",
        "Luck",
    ];

    /// Stats in on-disk order, matching [`Self::NAMES`].
    pub fn as_array_mut(&mut self) -> [&mut u32; 8] {
        [
            &mut self.vitality,
            &mut self.intelligence,
            &mut self.endurance,
            &mut self.strength,
            &mut self.dexterity,
            &mut self.magic,
            &mut self.faith,
            &mut self.luck,
        ]
    }

    pub fn as_array(&self) -> [u32; 8] {
        [
            self.vitality,
            self.intelligence,
            self.endurance,
            self.strength,
            self.dexterity,
            self.magic,
            self.faith,
            self.luck,
        ]
    }
}

/// Tendency values: positive is white, negative is black.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tendencies {
    pub character: f32,
    pub nexus: f32,
    pub boletarian_palace: f32,
    pub stonefang_tunnel: f32,
    pub tower_of_latria: f32,
    pub shrine_of_storms: f32,
    pub valley_of_defilement: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    /// Read-only for now: the name is also mirrored in the secondary
    /// (04/104/204USER.DAT) file, which we don't write yet.
    pub name: String,
    pub souls: u32,
    pub soul_memory: u32,
    pub levels_purchased: u32,
    pub stats: Stats,
    pub tendencies: Tendencies,
    pub clear_count: u8,
    pub inventory: Vec<InventoryItem>,
    pub stockpile: Vec<StockpileItem>,
}

pub(super) fn read_u32(b: &[u8], off: usize) -> u32 {
    u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
}

pub(super) fn write_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_be_bytes());
}

fn read_f32(b: &[u8], off: usize) -> f32 {
    f32::from_be_bytes(b[off..off + 4].try_into().unwrap())
}

fn write_f32(b: &mut [u8], off: usize, v: f32) {
    b[off..off + 4].copy_from_slice(&v.to_be_bytes());
}

fn read_name(b: &[u8]) -> String {
    let units: Vec<u16> = b[off::NAME..off::NAME + off::NAME_MAX_UNITS * 2]
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

const WORLD_TENDENCY_OFFSETS: [usize; 6] = [
    off::NEXUS_TENDENCY,
    off::W1_TENDENCY,
    off::W2_TENDENCY,
    off::W3_TENDENCY,
    off::W4_TENDENCY,
    off::W5_TENDENCY,
];

impl Tendencies {
    fn worlds(&self) -> [f32; 6] {
        [
            self.nexus,
            self.boletarian_palace,
            self.stonefang_tunnel,
            self.tower_of_latria,
            self.shrine_of_storms,
            self.valley_of_defilement,
        ]
    }
}

impl Character {
    pub fn read(bytes: &[u8]) -> Result<Self, UserDatError> {
        if bytes.len() < MIN_SIZE {
            return Err(UserDatError::TooSmall(bytes.len()));
        }
        if read_u32(bytes, off::SANITY_CHECK) == 0 {
            return Err(UserDatError::EmptySlot);
        }

        let stat = |i: usize| read_u32(bytes, off::STATS_BASE + i * off::STAT_PAIR_STRIDE);
        let stats = Stats {
            vitality: stat(0),
            intelligence: stat(1),
            endurance: stat(2),
            strength: stat(3),
            dexterity: stat(4),
            magic: stat(5),
            faith: stat(6),
            luck: stat(7),
        };

        let w = WORLD_TENDENCY_OFFSETS.map(|o| read_f32(bytes, o));
        let tendencies = Tendencies {
            character: read_f32(bytes, off::CHAR_TENDENCY),
            nexus: w[0],
            boletarian_palace: w[1],
            stonefang_tunnel: w[2],
            tower_of_latria: w[3],
            shrine_of_storms: w[4],
            valley_of_defilement: w[5],
        };

        Ok(Self {
            name: read_name(bytes),
            souls: read_u32(bytes, off::SOULS),
            soul_memory: read_u32(bytes, off::SOUL_MEMORY),
            levels_purchased: read_u32(bytes, off::LEVELS_PURCHASED),
            stats,
            tendencies,
            clear_count: bytes[off::CLEAR_COUNT],
            inventory: inventory::read(bytes)?,
            stockpile: stockpile::read(bytes)?,
        })
    }

    /// Patches the editable fields into `bytes`, leaving everything else as-is.
    /// On error `bytes` may be partially modified, so callers patch a copy.
    pub fn write_into(&self, bytes: &mut [u8]) -> Result<(), UserDatError> {
        if bytes.len() < MIN_SIZE {
            return Err(UserDatError::TooSmall(bytes.len()));
        }

        write_u32(bytes, off::SOULS, self.souls);
        write_u32(bytes, off::SOUL_MEMORY, self.soul_memory);
        write_u32(bytes, off::LEVELS_PURCHASED, self.levels_purchased);

        for (i, v) in self.stats.as_array().into_iter().enumerate() {
            let base = off::STATS_BASE + i * off::STAT_PAIR_STRIDE;
            write_u32(bytes, base, v);
            write_u32(bytes, base + 4, v);
        }

        write_f32(bytes, off::CHAR_TENDENCY, self.tendencies.character);
        for (o, v) in WORLD_TENDENCY_OFFSETS
            .into_iter()
            .zip(self.tendencies.worlds())
        {
            write_f32(bytes, o, v);
            write_f32(bytes, o + off::MIRROR, v);
        }

        bytes[off::CLEAR_COUNT] = self.clear_count;
        inventory::write(&self.inventory, bytes)?;
        stockpile::write(&self.stockpile, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_save() -> Vec<u8> {
        let mut b = vec![0u8; MIN_SIZE];
        write_u32(&mut b, off::SANITY_CHECK, 1);
        // Empty inventory: every record's type and the equip pointers are 0xFFFFFFFF.
        for slot in 0..inventory::SLOTS {
            write_u32(&mut b, 0x2DC + slot * 0x20, 0xFFFF_FFFF);
        }
        for o in (0x238..=0x27C).step_by(4) {
            write_u32(&mut b, o, 0xFFFF_FFFF);
        }
        // Empty stockpile: every entry's type byte is 0xFF.
        for slot in 0..stockpile::SLOTS {
            b[0x14BE8 + slot * 0x14 + 4] = 0xFF;
        }
        b
    }

    #[test]
    fn rejects_small_and_empty() {
        assert_eq!(Character::read(&[0; 16]), Err(UserDatError::TooSmall(16)));
        assert_eq!(
            Character::read(&vec![0; MIN_SIZE]),
            Err(UserDatError::EmptySlot)
        );
    }

    #[test]
    fn reads_name() {
        let mut b = blank_save();
        for (i, u) in "Borb".encode_utf16().enumerate() {
            b[off::NAME + i * 2..off::NAME + i * 2 + 2].copy_from_slice(&u.to_be_bytes());
        }
        assert_eq!(Character::read(&b).unwrap().name, "Borb");
    }

    #[test]
    fn roundtrip_and_mirrors() {
        let mut b = blank_save();
        let mut c = Character::read(&b).unwrap();
        c.souls = 123_456;
        c.soul_memory = 999;
        c.levels_purchased = 40;
        c.stats.strength = 32;
        c.stats.luck = 7;
        c.tendencies.character = -0.5;
        c.tendencies.stonefang_tunnel = 1.0;
        c.clear_count = 2;
        c.write_into(&mut b).unwrap();

        assert_eq!(Character::read(&b).unwrap(), c);
        // Base and effective stat values are both written.
        assert_eq!(read_u32(&b, 0x94), 32);
        assert_eq!(read_u32(&b, 0x98), 32);
        // World tendency mirror is written.
        assert_eq!(read_f32(&b, off::W2_TENDENCY + off::MIRROR), 1.0);
    }

    #[test]
    fn write_preserves_unrelated_bytes() {
        let mut b = blank_save();
        b[0x100] = 0xAB;
        b[MIN_SIZE - 1] = 0xCD;
        let before = b.clone();
        Character::read(&b).unwrap().write_into(&mut b).unwrap();
        assert_eq!(b, before);
    }
}
