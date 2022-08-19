use bytesize::ByteSize;
use meru_interface::{ConfigUi, CoreInfo, EmulatorCore, KeyConfig};
use serde::{Deserialize, Serialize};

use crate::{
    consts,
    context::{self, MemoryController},
    rom::{self, RomError, RomFormat},
    util::{Input, Pad},
};

pub struct Nes {
    pub ctx: context::Context,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {}

impl ConfigUi for Config {
    fn ui(&mut self, ui: &mut impl meru_interface::Ui) {
        ui.label("No config options");
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    RomError(#[from] RomError),
    #[error("unsupported mapper: {0}")]
    UnsupportedMapper(u16),
    #[error("{0}")]
    DeserializeFailed(#[from] bincode::Error),
    #[error("backup ram size mismatch: actual: {0}, expected: {1}")]
    BackupSizeMismatch(usize, usize),
}

const CORE_INFO: &'static CoreInfo = &CoreInfo {
    system_name: "NES (Sabicom)",
    abbrev: "nes",
    file_extensions: &["nes"],
};

fn default_key_config() -> KeyConfig {
    use meru_interface::key_assign::*;

    #[rustfmt::skip]
    let keys = vec![
        ("Up", any!(keycode!(Up), pad_button!(0, DPadUp))),
        ("Down", any!(keycode!(Down), pad_button!(0, DPadDown))),
        ("Left", any!(keycode!(Left), pad_button!(0, DPadLeft))),
        ("Right", any!(keycode!(Right), pad_button!(0, DPadRight))),
        ("A", any!(keycode!(X), pad_button!(0, South))),
        ("B", any!(keycode!(Z), pad_button!(0, West))),
        ("Start", any!(keycode!(Return), pad_button!(0, Start))),
        ("Select", any!(keycode!(RShift), pad_button!(0, Select))),
    ];

    let empty = vec![
        ("Up", KeyAssign::default()),
        ("Down", KeyAssign::default()),
        ("Left", KeyAssign::default()),
        ("Right", KeyAssign::default()),
        ("A", KeyAssign::default()),
        ("B", KeyAssign::default()),
        ("Start", KeyAssign::default()),
        ("Select", KeyAssign::default()),
    ];

    KeyConfig {
        controllers: [keys, empty]
            .into_iter()
            .map(|v| v.into_iter().map(|(k, a)| (k.to_string(), a)).collect())
            .collect(),
    }
}

impl EmulatorCore for Nes {
    type Config = Config;
    type Error = Error;

    fn core_info() -> &'static meru_interface::CoreInfo {
        &CORE_INFO
    }

    fn try_from_file(
        data: &[u8],
        backup: Option<&[u8]>,
        _config: &Self::Config,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        use context::Cpu;
        let rom = rom::Rom::from_bytes(data)?;
        let mut ctx = context::Context::new(rom, backup.map(|r| r.to_vec()))?;
        ctx.reset_cpu();
        Ok(Self { ctx })
    }

    fn game_info(&self) -> Vec<(String, String)> {
        use context::Rom;
        let rom = self.ctx.rom();

        let to_si = |x| ByteSize(x as _).to_string_as(true);
        let yn = |b| if b { "Yes" } else { "No" };

        let prg_chr_crc32 = {
            let mut hasher = crc32fast::Hasher::new();
            hasher.update(&rom.prg_rom);
            hasher.update(&rom.chr_rom);
            hasher.finalize()
        };
        let prg_rom_crc32 = crc32fast::hash(&rom.prg_rom);
        let chr_rom_crc32 = crc32fast::hash(&rom.chr_rom);

        let ret = vec![
            (
                "ROM Format",
                match &rom.format {
                    RomFormat::INes => "iNES",
                    RomFormat::Nes20 => "NES 2.0",
                }
                .to_string(),
            ),
            (
                "Mapper ID",
                format!("{} ({})", rom.mapper_id, rom.submapper_id),
            ),
            ("Mirroring", format!("{:?}", rom.mirroring)),
            ("Console Type", format!("{:?}", rom.console_type)),
            ("Timing Mode", format!("{:?}", rom.timing_mode)),
            ("Battery", yn(rom.has_battery).to_string()),
            ("Trainer", yn(rom.trainer.is_some()).to_string()),
            ("PRG ROM Size", to_si(rom.prg_rom.len())),
            ("CHR ROM Size", to_si(rom.chr_rom.len())),
            ("PRG RAM Size", to_si(rom.prg_ram_size)),
            ("PRG NVRAM Size", to_si(rom.prg_nvram_size)),
            ("CHR RAM Size", to_si(rom.chr_ram_size)),
            ("CHR NVRAM Size", to_si(rom.chr_nvram_size)),
            ("PRG+CHR CRC32", format!("{prg_chr_crc32:08X}")),
            ("PRG ROM CRC32", format!("{prg_rom_crc32:08X}")),
            ("CHR ROM CRC32", format!("{chr_rom_crc32:08X}")),
        ];

        ret.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    fn set_config(&mut self, _config: &Self::Config) {}

    fn exec_frame(&mut self, render_graphics: bool) {
        use context::{Apu, Cpu, Ppu};

        self.ctx.apu_mut().audio_buffer_mut().samples.clear();
        self.ctx
            .ppu_mut()
            .frame_buffer_mut()
            .resize(consts::SCREEN_WIDTH, consts::SCREEN_HEIGHT);
        self.ctx.ppu_mut().set_render_graphics(render_graphics);

        let frame = self.ctx.ppu().frame();
        while frame == self.ctx.ppu().frame() {
            self.ctx.tick_cpu();
        }
    }

    fn reset(&mut self) {
        use context::{Cpu, Rom};

        let backup = self.backup();
        let mut rom = rom::Rom::default();
        std::mem::swap(&mut rom, self.ctx.rom_mut());
        self.ctx = context::Context::new(rom, backup).unwrap();

        self.ctx.reset_cpu();
    }

    fn frame_buffer(&self) -> &meru_interface::FrameBuffer {
        use context::Ppu;
        self.ctx.ppu().frame_buffer()
    }

    fn audio_buffer(&self) -> &meru_interface::AudioBuffer {
        use context::Apu;
        self.ctx.apu().audio_buffer()
    }

    fn default_key_config() -> meru_interface::KeyConfig {
        default_key_config()
    }

    fn set_input(&mut self, input: &meru_interface::InputData) {
        let mut pad: [Pad; 2] = Default::default();

        for i in 0..2 {
            let mut pad = &mut pad[i];
            for (key, value) in &input.controllers[i] {
                match key.as_str() {
                    "Up" => pad.up = *value,
                    "Down" => pad.down = *value,
                    "Left" => pad.left = *value,
                    "Right" => pad.right = *value,
                    "A" => pad.a = *value,
                    "B" => pad.b = *value,
                    "Start" => pad.start = *value,
                    "Select" => pad.select = *value,
                    _ => (),
                }
            }
        }

        use context::Apu;
        self.ctx.apu_mut().set_input(&Input { pad });
    }

    fn backup(&self) -> Option<Vec<u8>> {
        use context::Rom;
        if self.ctx.rom().has_battery {
            Some(self.ctx.memory_ctrl().prg_ram().to_vec())
        } else {
            None
        }
    }

    fn save_state(&self) -> Vec<u8> {
        bincode::serialize(&self.ctx).unwrap()
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        use context::{Apu, Ppu, Rom};
        let mut ctx: context::Context = bincode::deserialize(data)?;
        std::mem::swap(ctx.rom_mut(), self.ctx.rom_mut());
        std::mem::swap(
            ctx.ppu_mut().frame_buffer_mut(),
            self.ctx.ppu_mut().frame_buffer_mut(),
        );
        std::mem::swap(
            ctx.apu_mut().audio_buffer_mut(),
            self.ctx.apu_mut().audio_buffer_mut(),
        );
        self.ctx = ctx;
        Ok(())
    }
}
