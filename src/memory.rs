use serde::{Deserialize, Serialize};

use crate::{
    context,
    rom::{Mirroring, Rom},
    util::trait_alias,
};

trait_alias!(pub trait Context = context::Mapper + context::Ppu + context::Apu + context::Interrupt + context::Timing);

#[derive(Serialize, Deserialize)]
pub struct MemoryMap {
    ram: Vec<u8>,
    cpu_stall: u64,
}

impl MemoryMap {
    pub fn new() -> Self {
        Self {
            ram: vec![0x00; 2 * 1024],
            cpu_stall: 0,
        }
    }

    pub fn read(&self, ctx: &mut impl Context, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize],
            0x2000..=0x3fff => ctx.read_ppu(addr & 7),
            0x4000..=0x4017 => ctx.read_apu(addr),
            0x4018..=0xffff => ctx.read_prg(addr),
        }
    }

    pub fn read_pure(&self, ctx: &impl Context, addr: u16) -> Option<u8> {
        Some(match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize],
            0x2000..=0x3fff => None?,
            0x4000..=0x4017 => None?,
            0x4018..=0xffff => ctx.read_prg(addr),
        })
    }

    pub fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize] = data,
            0x2000..=0x3fff => ctx.write_ppu(addr & 7, data),
            0x4000..=0x4013 | 0x4015..=0x4017 => ctx.write_apu(addr, data),
            0x4018..=0xffff => ctx.write_prg(addr, data),

            0x4014 => {
                // OAM DMA
                let hi = (data as u16) << 8;

                for lo in 0..0x100 {
                    let data = self.read(ctx, hi | lo);
                    self.write(ctx, 0x2004, data);
                }

                // FIXME: odd frame stall one more cycle
                self.cpu_stall += 513
            }
        }
    }

    pub fn tick(&mut self, ctx: &mut impl Context) {
        for _ in 0..3 {
            ctx.tick_ppu();
            ctx.tick_mapper();
        }
        ctx.tick_apu();
    }

    pub fn cpu_stall(&mut self) -> u64 {
        let ret = self.cpu_stall;
        self.cpu_stall = 0;
        ret
    }
}

#[derive(Serialize, Deserialize)]
pub struct MemoryController {
    prg_ram: Vec<u8>,
    chr_ram: Vec<u8>,

    nametable: Vec<u8>,
    palette: [u8; 0x20],

    rom_page: [usize; 4],
    chr_page: [usize; 8],
    nametable_page: [usize; 4],
}

impl MemoryController {
    pub fn new(rom: &Rom) -> Self {
        assert!(!(rom.chr_ram_size > 0 && !rom.chr_rom.is_empty()));

        let mirroring = rom.mirroring;

        let prg_ram = vec![0x00; rom.prg_ram_size];
        let chr_ram = vec![0x00; rom.chr_ram_size];

        let nametable = vec![0x00; 2 * 1024];

        #[rustfmt::skip]
        let palette = [
            0x09, 0x01, 0x00, 0x01, 0x00, 0x02, 0x02, 0x0D,
            0x08, 0x10, 0x08, 0x24, 0x00, 0x00, 0x04, 0x2C,
            0x09, 0x01, 0x34, 0x03, 0x00, 0x04, 0x00, 0x14,
            0x08, 0x3A, 0x00, 0x02, 0x00, 0x20, 0x2C, 0x08,
        ];

        let mut ret = Self {
            prg_ram,
            chr_ram,
            nametable,
            palette,
            rom_page: [0; 4],
            chr_page: [0; 8],
            nametable_page: [0; 4],
        };

        for i in 0..4 {
            ret.map_prg(rom, i, i);
        }

        for i in 0..8 {
            ret.map_chr(rom, i, i);
        }

        ret.set_mirroring(mirroring);

        ret
    }

    /// Maps a PRG ROM page to a given 8KB bank
    pub fn map_prg(&mut self, rom: &Rom, page: usize, bank: usize) {
        self.rom_page[page] = (bank * 0x2000) % rom.prg_rom.len();
    }

    pub fn prg_pages(&mut self, rom: &Rom) -> usize {
        rom.prg_rom.len() / 0x2000
    }

    pub fn prg_page(&self, page: u16) -> u16 {
        (self.rom_page[page as usize] / 0x2000) as u16
    }

    /// Maps a CHR ROM page to a given 1KB bank
    pub fn map_chr(&mut self, rom: &Rom, page: usize, bank: usize) {
        if !rom.chr_rom.is_empty() {
            self.chr_page[page] = (bank * 0x0400) % rom.chr_rom.len();
        } else {
            self.chr_page[page] = (bank * 0x0400) % rom.chr_ram_size;
        }
    }

    pub fn chr_pages(&mut self, rom: &Rom) -> usize {
        rom.chr_rom.len() / 0x0400
    }

    pub fn map_nametable(&mut self, page: usize, bank: usize) {
        self.nametable_page[page] = bank * 0x0400;
    }

    pub fn set_mirroring(&mut self, mirroring: Mirroring) {
        match mirroring {
            Mirroring::OneScreenLow => {
                self.map_nametable(0, 0);
                self.map_nametable(1, 0);
                self.map_nametable(2, 0);
                self.map_nametable(3, 0);
            }
            Mirroring::OneScreenHigh => {
                self.map_nametable(0, 1);
                self.map_nametable(1, 1);
                self.map_nametable(2, 1);
                self.map_nametable(3, 1);
            }
            Mirroring::Horizontal => {
                self.map_nametable(0, 0);
                self.map_nametable(1, 0);
                self.map_nametable(2, 1);
                self.map_nametable(3, 1);
            }
            Mirroring::Vertical => {
                self.map_nametable(0, 0);
                self.map_nametable(1, 1);
                self.map_nametable(2, 0);
                self.map_nametable(3, 1);
            }
            Mirroring::FourScreen => {
                todo!()
            }
        }
    }

    pub fn read_prg(&self, rom: &Rom, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7fff => {
                let addr = addr & 0x1fff;
                self.prg_ram[addr as usize]
            }
            0x8000..=0xffff => {
                let page = (addr & 0x7fff) / 0x2000;
                let ix = self.rom_page[page as usize] + (addr & 0x1fff) as usize;
                rom.prg_rom[ix]
            }
            _ => 0,
        }
    }

    pub fn write_prg(&mut self, rom: &Rom, addr: u16, data: u8) {
        match addr {
            0x6000..=0x7fff => {
                let addr = addr & 0x1fff;
                self.prg_ram[addr as usize] = data;
            }
            0x8000..=0xffff => {
                log::warn!("Write to PRG ROM: {addr:04x} = {data:02x}");
            }
            _ => (),
        }
    }

    pub fn read_chr(&self, rom: &Rom, addr: u16) -> u8 {
        log::trace!("Read CHR MEM: ${addr:04X}");

        match addr {
            0x0000..=0x1fff => {
                let page = (addr / 0x0400) as usize;
                let ix = self.chr_page[page] + (addr & 0x03ff) as usize;

                if !rom.chr_rom.is_empty() {
                    rom.chr_rom[ix]
                } else {
                    self.chr_ram[ix]
                }
            }
            0x2000..=0x3eff => {
                let page = (addr as usize & 0x0fff) / 0x400;
                let ofs = addr as usize & 0x03ff;
                let ix = self.nametable_page[page] + ofs;
                self.nametable[ix]
            }
            0x3f00..=0x3fff => {
                let addr = addr & if addr & 3 == 0 { 0x0f } else { 0x1f };
                self.palette[addr as usize]
            }
            _ => unreachable!(),
        }
    }

    pub fn write_chr(&mut self, rom: &Rom, addr: u16, data: u8) {
        log::trace!("Write CHR MEM: (${addr:04X}) = ${data:02X}");

        match addr {
            0x0000..=0x1fff => {
                let page = (addr / 0x0400) as usize;
                let ix = self.chr_page[page] + (addr & 0x03ff) as usize;

                if !rom.chr_rom.is_empty() {
                    log::warn!("Write to CHR ROM: (${addr:04X}) = ${data:02X}");
                } else {
                    self.chr_ram[ix] = data;
                }
            }
            0x2000..=0x3eff => {
                let page = (addr as usize & 0x0fff) / 0x400;
                let ofs = addr as usize & 0x03ff;
                let ix = self.nametable_page[page] + ofs;
                self.nametable[ix] = data;
            }
            0x3f00..=0x3fff => {
                let addr = addr & if addr & 3 == 0 { 0x0f } else { 0x1f };
                self.palette[addr as usize] = data;
            }
            _ => unreachable!(),
        }
    }
}
