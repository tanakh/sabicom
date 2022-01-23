use crate::{memory::MemoryController, rom::Rom, util::Ref};

pub struct Cnrom {
    ctrl: MemoryController,
}

impl Cnrom {
    pub fn new(rom: Ref<Rom>) -> Self {
        let mut ctrl = MemoryController::new(rom);
        for i in 0..4 {
            ctrl.map_prg(i, i);
        }
        for i in 0..8 {
            ctrl.map_chr(i, i);
        }
        Self { ctrl }
    }
}

impl super::Mapper for Cnrom {
    fn read_prg(&mut self, addr: u16) -> u8 {
        self.ctrl.read_prg(addr)
    }

    fn read_chr(&mut self, addr: u16) -> u8 {
        self.ctrl.read_chr(addr)
    }

    fn write_prg(&mut self, _addr: u16, data: u8) {
        for i in 0..8 {
            self.ctrl.map_chr(i, data as usize * 8 + i);
        }
    }

    fn write_chr(&mut self, addr: u16, data: u8) {
        self.ctrl.write_chr(addr, data);
    }
}
