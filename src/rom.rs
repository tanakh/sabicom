use serde::{Deserialize, Serialize};

pub struct Rom {
    pub format: RomFormat,
    pub mapper_id: u16,
    pub submapper_id: u8,
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub trainer: Option<Vec<u8>>,
    pub prg_ram_size: usize,
    pub prg_nvram_size: usize,
    pub chr_ram_size: usize,
    pub chr_nvram_size: usize,
    pub mirroring: Mirroring,
    pub console_type: ConsoleType,
    pub timing_mode: TimingMode,
    pub has_battery: bool,
}

impl Default for Rom {
    fn default() -> Self {
        Self {
            format: RomFormat::INes,
            mapper_id: 0,
            submapper_id: 0,
            prg_rom: vec![],
            chr_rom: vec![],
            trainer: None,
            prg_ram_size: 0,
            prg_nvram_size: 0,
            chr_ram_size: 0,
            chr_nvram_size: 0,
            mirroring: Mirroring::Vertical,
            console_type: ConsoleType::Nes,
            timing_mode: TimingMode::Ntsc,
            has_battery: false,
        }
    }
}

pub enum RomFormat {
    INes,
    Nes20,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mirroring {
    OneScreenLow,
    OneScreenHigh,
    Horizontal,
    Vertical,
    FourScreen,
}

#[derive(Debug)]
pub enum ConsoleType {
    Nes,
    VsSystem { ppu_type: u8, hardware_type: u8 },
    Playchoice10,
    ExtendConsoleType { console_type: u8 },
}

#[derive(Debug)]
pub enum TimingMode {
    Ntsc,
    Pal,
    MultipleRegion,
    Dendy,
}

#[derive(thiserror::Error, Debug)]
pub enum RomError {
    #[error("invalid ROM magic: {0:?}, expected: 'NES\x1a'")]
    InvalidMagic([u8; 4]),
    #[error("Invalid mirroring: {0}")]
    InvalidMirroring(u8),
    #[error("ROM data has invalid extra bytes")]
    InvalidExtraBytes,
}

impl Rom {
    pub fn from_bytes(dat: &[u8]) -> Result<Self, RomError> {
        let header = &dat[..0x10];
        let mut dat = &dat[0x10..];

        let magic = &header[0..4];
        if magic != b"NES\x1a" {
            Err(RomError::InvalidMagic(magic.try_into().unwrap()))?;
        }

        let is_nes2 = header[7] & 0x0C == 0x08;

        let prg_rom_size_in_16kib = if is_nes2 {
            header[4] as usize | (header[9] as usize & 0x0f) << 8
        } else {
            header[4] as usize
        };

        let prg_rom_size = prg_rom_size_in_16kib * 16 * 1024;

        let chr_rom_size_in_8kib = if is_nes2 {
            header[5] as usize | (header[9] as usize >> 4) << 8
        } else {
            header[5] as usize
        };

        let chr_rom_size = chr_rom_size_in_8kib * 8 * 1024;

        let mirroring = match header[6] & 0x09 {
            0 => Mirroring::Horizontal,
            1 => Mirroring::Vertical,
            8 => Mirroring::FourScreen,
            _ => Err(RomError::InvalidMirroring(header[6] & 0x09))?,
        };

        let has_battery = header[6] & 0x02 != 0;
        let has_trainer = header[6] & 0x04 != 0;

        let mapper_id = if is_nes2 {
            header[6] as u16 >> 4 | header[7] as u16 & 0xf0 | (header[8] as u16 & 0xf) << 8
        } else {
            header[6] as u16 >> 4 | header[7] as u16 & 0xf0
        };

        let submapper_id = if is_nes2 { header[8] >> 4 } else { 0 };

        let console_type = if is_nes2 {
            match header[7] & 3 {
                0 => ConsoleType::Nes,
                1 => ConsoleType::VsSystem {
                    ppu_type: header[13] & 0x0f,
                    hardware_type: header[13] >> 4,
                },
                2 => ConsoleType::Playchoice10,
                3 => ConsoleType::ExtendConsoleType {
                    console_type: header[13] & 0x0f,
                },
                _ => unreachable!(),
            }
        } else {
            ConsoleType::Nes
        };

        let prg_ram_size = if is_nes2 {
            let shift_count = header[10] & 0xf;
            if shift_count == 0 {
                0
            } else {
                64 << shift_count
            }
        } else if header[8] == 0 {
            8 * 1024
        } else {
            header[8] as usize * 8 * 1024
        };

        let prg_nvram_size = if is_nes2 {
            let shift_count = header[10] >> 4;
            if shift_count == 0 {
                0
            } else {
                64 << shift_count
            }
        } else {
            0
        };

        let chr_ram_size = if is_nes2 {
            let shift_count = header[11] & 0xf;
            if shift_count == 0 {
                0
            } else {
                64 << shift_count
            }
        } else if chr_rom_size == 0 {
            8 * 1024
        } else {
            0
        };

        let chr_nvram_size = if is_nes2 {
            let shift_count = header[11] >> 4;
            if shift_count == 0 {
                0
            } else {
                64 << shift_count
            }
        } else {
            0
        };

        let timing_mode = if is_nes2 {
            match header[12] & 3 {
                0 => TimingMode::Ntsc,
                1 => TimingMode::Pal,
                2 => TimingMode::MultipleRegion,
                3 => TimingMode::Dendy,
                _ => unreachable!(),
            }
        } else {
            match header[10] & 3 {
                0 => TimingMode::Ntsc,
                2 => TimingMode::Pal,
                _ => TimingMode::MultipleRegion,
            }
        };

        // TODO:

        //  14     Miscellaneous ROMs
        //         D~7654 3210
        //           ---------
        //           .... ..RR
        //                  ++- Number of miscellaneous ROMs present

        //  15     Default Expansion Device
        //         D~7654 3210
        //           ---------
        //           ..DD DDDD
        //             ++-++++- Default Expansion Device

        let trainer = if has_trainer {
            let v = &dat[..512];
            dat = &dat[512..];
            Some(v.to_owned())
        } else {
            None
        };

        let prg_rom = dat[..prg_rom_size].to_owned();
        dat = &dat[prg_rom_size..];
        let chr_rom = dat[..chr_rom_size].to_owned();
        dat = &dat[chr_rom_size..];

        if !dat.is_empty() {
            Err(RomError::InvalidExtraBytes)?;
        }

        let format = if is_nes2 {
            RomFormat::Nes20
        } else {
            RomFormat::INes
        };

        Ok(Self {
            format,
            prg_rom,
            chr_rom,
            trainer,
            mapper_id,
            submapper_id,
            mirroring,
            console_type,
            timing_mode,
            has_battery,
            prg_ram_size,
            prg_nvram_size,
            chr_ram_size,
            chr_nvram_size,
        })
    }
}
