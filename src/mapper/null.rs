use std::{cell::RefCell, rc::Rc};

use crate::rom::Rom;

pub struct NullMapper {
    rom: Rc<RefCell<Rom>>,

    rom_page: [usize; 4],
    chr_page: [usize; 8],
}

impl NullMapper {
    pub fn new(rom: Rc<RefCell<Rom>>) -> Self {
        let mut ret = Self {
            rom,
            rom_page: [0; 4],
            chr_page: [0; 8],
        };

        for i in 0..4 {
            ret.map_rom(i, i);
        }

        for i in 0..8 {
            ret.map_chr(i, i);
        }

        ret
    }

    fn map_rom(&mut self, page: usize, val: usize) {
        self.rom_page[page] = (val * 0x2000) % self.rom.borrow().prg_rom.len();
    }

    fn map_chr(&mut self, page: usize, val: usize) {
        self.chr_page[page] = (val * 0x0400) % self.rom.borrow().chr_rom.len();
    }
}

impl super::Mapper for NullMapper {
    fn read_u8(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xffff => {
                let page = (addr - 0x8000) / 0x2000;
                let ix = self.rom_page[page as usize] + (addr & 0x1fff) as usize;
                self.rom.borrow().prg_rom[ix]
            }

            _ => 0,
        }
    }

    fn write_u8(&mut self, _addr: u16, _val: u8) {}
}
