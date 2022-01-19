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

    fn write_prg(&mut self, addr: u16, data: u8) {
        log::warn!("write to PRG: {addr:04X} {data:02X}");
    }

    fn write_chr(&mut self, addr: u16, data: u8) {
        self.ctrl.write_chr(addr, data);
    }
}
