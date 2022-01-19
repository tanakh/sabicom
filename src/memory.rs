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

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize],
            0x2000..=0x3fff => self.ppu.borrow_mut().read_reg(addr & 7),
            0x4000..=0x4017 => self.apu.borrow_mut().read_reg(addr),
            0x4018..=0xffff => self.mapper.borrow_mut().read_prg(addr),
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize] = data,
            0x2000..=0x3fff => self.ppu.borrow_mut().write_reg(addr & 7, data),
            0x4000..=0x4017 => self.apu.borrow_mut().write_reg(addr, data),
            0x4018..=0xffff => self.mapper.borrow_mut().write_prg(addr, data),
        }
    }
}
