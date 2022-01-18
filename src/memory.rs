use crate::{apu::Apu, mapper::Mapper, ppu::Ppu, util::Ref};

pub struct MemoryMap {
    ram: Vec<u8>,
    ppu: Ref<Ppu>,
    apu: Ref<Apu>,
    mapper: Ref<dyn Mapper>,
}

impl MemoryMap {
    pub fn new(ppu: Ref<Ppu>, apu: Ref<Apu>, mapper: Ref<dyn Mapper>) -> Self {
        Self {
            ram: vec![0x00; 2 * 1024],
            ppu,
            apu,
            mapper,
        }
    }

    pub fn read_u8(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize],
            0x2000..=0x3fff => self.ppu.borrow_mut().read_reg(addr & 7),
            0x4000..=0x401f => self.apu.borrow_mut().read_reg(addr),
            0x4018..=0xffff => self.mapper.borrow_mut().read_prg(addr),
        }
    }

    pub fn write_u8(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize] = val,
            0x2000..=0x3fff => self.ppu.borrow_mut().write_reg(addr & 7, val),
            0x4000..=0x401f => self.apu.borrow_mut().write_reg(addr, val),
            0x4018..=0xffff => self.mapper.borrow_mut().write_prg(addr, val),
        }
    }

    pub fn read_u16(&self, addr: u16) -> u16 {
        (self.read_u8(addr) as u16) | ((self.read_u8(addr + 1) as u16) << 8)
    }

    pub fn write_u16(&mut self, addr: u16, val: u16) {
        self.write_u8(addr, (val & 0xff) as u8);
        self.write_u8(addr + 1, (val >> 8) as u8);
    }
}
