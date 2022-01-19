mod mmc1;
mod null;

use crate::{
    rom::Rom,
    util::{wrap_ref, Ref},
};

pub trait Mapper {
    fn read_prg(&mut self, addr: u16) -> u8;
    fn write_prg(&mut self, addr: u16, val: u8);

    fn read_chr(&mut self, addr: u16) -> u8;
    fn write_chr(&mut self, addr: u16, val: u8);
}

pub fn create_mapper(rom: Ref<Rom>) -> Ref<dyn Mapper> {
    let mapper_id = rom.borrow().mapper_id;

    match mapper_id {
        0 => wrap_ref(null::NullMapper::new(rom)),
        1 => wrap_ref(mmc1::Mmc1::new(rom)),
        _ => panic!("Unsupported mapper: {mapper_id}"),
    }
}

pub struct MemoryController {
    rom: Ref<Rom>,

    prg_ram: Vec<u8>,
    chr_ram: Vec<u8>,

    nametable: Vec<u8>,
    palette: Vec<u8>,

    rom_page: [usize; 4],
    chr_page: [usize; 8],
    nametable_page: [usize; 4],
}

impl MemoryController {
    fn new(rom: Ref<Rom>) -> Self {
        assert!(!(rom.borrow().chr_ram_size > 0 && !rom.borrow().chr_rom.is_empty()));

        let prg_ram = vec![0x00; rom.borrow().prg_ram_size];
        let chr_ram = vec![0x00; rom.borrow().chr_ram_size];

        let nametable = vec![0x00; 2 * 1024];
        let palette = vec![0x00; 0x20];

        let mut ret = Self {
            rom,
            prg_ram,
            chr_ram,
            nametable,
            palette,
            rom_page: [0; 4],
            chr_page: [0; 8],
            nametable_page: [0; 4],
        };

        for i in 0..4 {
            ret.map_prg(i, i);
        }

        for i in 0..8 {
            ret.map_chr(i, i);
        }

        ret
    }

    /// Maps a PRG ROM page to a given 8KB bank
    fn map_prg(&mut self, page: usize, bank: usize) {
        self.rom_page[page] = (bank * 0x2000) % self.rom.borrow().prg_rom.len();
    }

    /// Maps a CHR ROM page to a given 1KB bank
    fn map_chr(&mut self, page: usize, bank: usize) {
        if !self.rom.borrow().chr_rom.is_empty() {
            self.chr_page[page] = (bank * 0x0400) % self.rom.borrow().chr_rom.len();
        } else {
            self.chr_page[page] = (bank * 0x0400) % self.rom.borrow().chr_ram_size;
        }
    }

    fn set_mirroring(&mut self, page: usize, bank: usize) {
        todo!()
    }

    fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xffff => {
                let page = (addr & 0x7fff) / 0x2000;
                let ix = self.rom_page[page as usize] + (addr & 0x1fff) as usize;
                self.rom.borrow().prg_rom[ix]
            }
            _ => 0,
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        log::info!("Read CHR MEM: ${addr:04X}");

        match addr {
            0x0000..=0x1fff => {
                let page = (addr / 0x0400) as usize;
                let ix = self.chr_page[page] + (addr & 0x03ff) as usize;

                if !self.rom.borrow().chr_rom.is_empty() {
                    self.rom.borrow().chr_rom[ix]
                } else {
                    self.chr_ram[ix]
                }
            }
            0x2000..=0x3eff => {
                let page = (addr as usize & 0x1fff) / 0x400;
                let ofs = addr as usize & 0x03ff;
                let ix = self.nametable_page[page] + ofs;
                self.nametable[ix]
            }
            0x3f00..=0x3fff => self.palette[(addr & 0x1f) as usize],
            _ => unreachable!(),
        }
    }

    fn write_chr(&mut self, addr: u16, val: u8) {
        log::info!("Write CHR MEM: (${addr:04X}) = ${val:02X}");

        match addr {
            0x0000..=0x1fff => {
                let page = (addr / 0x0400) as usize;
                let ix = self.chr_page[page] + (addr & 0x03ff) as usize;

                if !self.rom.borrow().chr_rom.is_empty() {
                    panic!("Write to CHR ROM: (${addr:04X}) = ${val:02X}");
                } else {
                    self.chr_ram[ix] = val;
                }
            }
            0x2000..=0x3eff => {
                let page = (addr as usize & 0x1fff) / 0x400;
                let ofs = addr as usize & 0x03ff;
                let ix = self.nametable_page[page] + ofs;
                self.nametable[ix] = val;
            }
            0x3f00..=0x3fff => {
                self.palette[(addr & 0x1f) as usize] = val;
            }
            _ => unreachable!(),
        }
    }
}
