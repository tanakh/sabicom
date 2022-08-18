use serde::{Deserialize, Serialize};

use crate::{memory::MemoryController, rom::Rom};

#[derive(Serialize, Deserialize)]
pub struct Unrom {
    ctrl: MemoryController,
}

impl Unrom {
    pub fn new(rom: &Rom) -> Self {
        let mut ctrl = MemoryController::new(rom);
        let prg_pages = ctrl.prg_pages(rom);
        ctrl.map_prg(rom, 0, 0);
        ctrl.map_prg(rom, 1, 1);
        ctrl.map_prg(rom, 2, prg_pages - 2);
        ctrl.map_prg(rom, 3, prg_pages - 1);
        Self { ctrl }
    }
}

impl super::MapperTrait for Unrom {
    fn read_prg(&self, ctx: &impl super::Context, addr: u16) -> u8 {
        self.ctrl.read_prg(ctx.rom(), addr)
    }

    fn write_prg(&mut self, ctx: &mut impl super::Context, addr: u16, data: u8) {
        self.ctrl.map_prg(ctx.rom(), 0, data as usize * 2);
        self.ctrl.map_prg(ctx.rom(), 1, data as usize * 2 + 1);
    }

    fn read_chr(&mut self, ctx: &mut impl super::Context, addr: u16) -> u8 {
        self.ctrl.read_chr(ctx.rom(), addr)
    }

    fn write_chr(&mut self, ctx: &mut impl super::Context, addr: u16, data: u8) {
        self.ctrl.write_chr(ctx.rom(), addr, data);
    }

    fn prg_page(&self, page: u16) -> u16 {
        self.ctrl.prg_page(page)
    }
}
