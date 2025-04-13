use std::io;

/// Magic number for .md files
const MD_MAGIC: &[u8; 4] = b"MD\0\0";
const MD_ENTRY_LEN: usize = 68;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokemonType {
    None = 0,
    Normal = 1,
    Fire = 2,
    Water = 3,
    Grass = 4,
    Electric = 5,
    Ice = 6,
    Fighting = 7,
    Poison = 8,
    Ground = 9,
    Flying = 10,
    Psychic = 11,
    Bug = 12,
    Rock = 13,
    Ghost = 14,
    Dragon = 15,
    Dark = 16,
    Steel = 17,
    Neutral = 18,
}

impl From<u8> for PokemonType {
    fn from(value: u8) -> Self {
        match value {
            1 => PokemonType::Normal,
            2 => PokemonType::Fire,
            3 => PokemonType::Water,
            4 => PokemonType::Grass,
            5 => PokemonType::Electric,
            6 => PokemonType::Ice,
            7 => PokemonType::Fighting,
            8 => PokemonType::Poison,
            9 => PokemonType::Ground,
            10 => PokemonType::Flying,
            11 => PokemonType::Psychic,
            12 => PokemonType::Bug,
            13 => PokemonType::Rock,
            14 => PokemonType::Ghost,
            15 => PokemonType::Dragon,
            16 => PokemonType::Dark,
            17 => PokemonType::Steel,
            18 => PokemonType::Neutral,
            _ => PokemonType::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowSize {
    Small = 0,
    Medium = 1,
    Large = 2,
}

impl From<i8> for ShadowSize {
    fn from(value: i8) -> Self {
        match value {
            0 => ShadowSize::Small,
            1 => ShadowSize::Medium,
            2 => ShadowSize::Large,
            _ => ShadowSize::Medium,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonsterStats {
    pub base_hp: u16,
    pub base_atk: u8,
    pub base_sp_atk: u8,
    pub base_def: u8, 
    pub base_sp_def: u8,
}

#[derive(Debug, Clone)]
pub struct MonsterEntry {
    pub md_index: u32,
    pub national_pokedex_number: u16,
    pub sprite_index: i16,
    pub stats: MonsterStats,
    pub type_primary: PokemonType,
    pub type_secondary: PokemonType,
    pub weight: i16,
    pub shadow_size: ShadowSize,
}

/// Container for all monster data entries
#[derive(Debug)]
pub struct MonsterData {
    pub entries: Vec<MonsterEntry>,
}

impl MonsterData {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        // Check magic number
        if data.len() < 8 || &data[0..4] != MD_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid magic number, expected MD\\0\\0",
            ));
        }

        let number_entries = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        
        let expected_size = 8 + (number_entries as usize * MD_ENTRY_LEN);
        if data.len() < expected_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Data too short, expected {} bytes", expected_size),
            ));
        }

        // Parse each entry
        let mut entries = Vec::with_capacity(number_entries as usize);
        for i in 0..number_entries {
            let start = 8 + (i as usize * MD_ENTRY_LEN);
            
            let national_pokedex_number = u16::from_le_bytes([data[start + 0x04], data[start + 0x05]]);
            let sprite_index = i16::from_le_bytes([data[start + 0x10], data[start + 0x11]]);
            let type_primary = PokemonType::from(data[start + 0x14]);
            let type_secondary = PokemonType::from(data[start + 0x15]);
            
            let base_hp = u16::from_le_bytes([data[start + 0x20], data[start + 0x21]]);
            let base_atk = data[start + 0x24];
            let base_sp_atk = data[start + 0x25];
            let base_def = data[start + 0x26];
            let base_sp_def = data[start + 0x27];
            
            let weight = i16::from_le_bytes([data[start + 0x28], data[start + 0x29]]);
            let shadow_size = ShadowSize::from(data[start + 0x2E] as i8);
            
            entries.push(MonsterEntry {
                md_index: i,
                national_pokedex_number,
                sprite_index,
                stats: MonsterStats {
                    base_hp,
                    base_atk,
                    base_sp_def,
                    base_def,
                    base_sp_atk,
                },
                type_primary,
                type_secondary,
                weight,
                shadow_size,
            });
        }

        Ok(Self { entries })
    }
}
