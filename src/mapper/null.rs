use crate::{rom::Rom, util::Ref};

pub struct NullMapper {
    ctrl: super::MemoryController,
}

impl NullMapper {
    pub fn new(rom: Ref<Rom>) -> Self {
        Self {
            ctrl: super::MemoryController::new(rom),
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

    fn write_prg(&mut self, _addr: u16, _val: u8) {
        todo!()
    }

    fn write_chr(&mut self, _addr: u16, _val: u8) {
        todo!()
    }
}
