use std::{cell::RefCell, rc::Rc};

use crate::{mapper::Mapper, ppu::Ppu};

pub struct MemoryMap {
    ram: Vec<u8>,
    ppu: Rc<RefCell<Ppu>>,
    mapper: Rc<RefCell<dyn Mapper>>,
}

impl MemoryMap {
    pub fn new(ppu: Rc<RefCell<Ppu>>, mapper: Rc<RefCell<dyn Mapper>>) -> Self {
        Self {
            ram: vec![0x00; 2 * 1024],
            ppu,
            mapper,
        }
    }

    pub fn read_u8(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize],
            0x2000..=0x3fff => self.ppu.borrow_mut().read_reg(addr & 7),
            0x4000..=0x401f => {
                todo!("APU and I/O registers")
            }
            0x4018..=0xffff => self.mapper.borrow().read_u8(addr),
        }
    }

    pub fn write_u8(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize] = val,
            0x2000..=0x3fff => self.ppu.borrow_mut().write_reg(addr & 7, val),
            0x4000..=0x401f => {
                todo!("APU and I/O registers")
            }
            0x4018..=0xffff => self.mapper.borrow_mut().write_u8(addr, val),
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
