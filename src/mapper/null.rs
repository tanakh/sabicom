use crate::{memory::MemoryController, rom::Rom, util::Ref};

pub struct NullMapper {
    ctrl: MemoryController,
}

impl NullMapper {
    pub fn new(rom: Ref<Rom>) -> Self {
        Self {
            ctrl: MemoryController::new(rom),
        }
    }
}

impl super::Mapper for NullMapper {
    fn read_prg(&mut self, addr: u16) -> u8 {
        self.ctrl.read_prg(addr)
    }

    fn read_chr(&mut self, addr: u16) -> u8 {
        self.ctrl.read_chr(addr)
    }

    fn write_prg(&mut self, addr: u16, data: u8) {
        self.ctrl.write_prg(addr, data);
    }

    fn write_chr(&mut self, addr: u16, data: u8) {
        self.ctrl.write_chr(addr, data);
    }
}
