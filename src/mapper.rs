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

    rom_page: [usize; 4],
    chr_page: [usize; 8],
}

impl MemoryController {
    fn new(rom: Ref<Rom>) -> Self {
        let mut ret = Self {
            rom,
            rom_page: [0; 4],
            chr_page: [0; 8],
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
        if self.rom.borrow().chr_rom.is_empty() {
            log::warn!("No CHR ROM found: page[{page}] = {bank}");
            return;
        }

        self.chr_page[page] = (bank * 0x0400) % self.rom.borrow().chr_rom.len();
    }

    fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xffff => {
                let page = (addr - 0x8000) / 0x2000;
                let ix = self.rom_page[page as usize] + (addr & 0x1fff) as usize;
                self.rom.borrow().prg_rom[ix]
            }
            _ => 0,
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        todo!("Read CHR ROM: {addr:04X}")
    }
}
