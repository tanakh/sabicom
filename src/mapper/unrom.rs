use crate::{memory::MemoryController, rom::Rom, util::Ref};

pub struct Unrom {
    ctrl: MemoryController,
}

impl Unrom {
    pub fn new(rom: Ref<Rom>) -> Self {
        let mut ctrl = MemoryController::new(rom);
        let prg_pages = ctrl.prg_pages();
        ctrl.map_prg(0, 0);
        ctrl.map_prg(1, 1);
        ctrl.map_prg(2, prg_pages - 2);
        ctrl.map_prg(3, prg_pages - 1);
        Self { ctrl }
    }
}

impl super::Mapper for Unrom {
    fn read_prg(&mut self, addr: u16) -> u8 {
        self.ctrl.read_prg(addr)
    }

    fn read_chr(&mut self, addr: u16) -> u8 {
        self.ctrl.read_chr(addr)
    }

    fn write_prg(&mut self, _addr: u16, data: u8) {
        self.ctrl.map_prg(0, data as usize * 2);
        self.ctrl.map_prg(1, data as usize * 2 + 1);
    }

    fn write_chr(&mut self, addr: u16, data: u8) {
        self.ctrl.write_chr(addr, data);
    }
}
