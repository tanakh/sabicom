use serde::{Deserialize, Serialize};

use crate::{memory::MemoryController, rom::Rom};

#[derive(Serialize, Deserialize)]
pub struct Cnrom {
    ctrl: MemoryController,
}

impl Cnrom {
    pub fn new(rom: &Rom) -> Self {
        let mut ctrl = MemoryController::new(rom);
        for i in 0..4 {
            ctrl.map_prg(rom, i, i);
        }
        for i in 0..8 {
            ctrl.map_chr(rom, i, i);
        }
        Self { ctrl }
    }
}

impl super::MapperTrait for Cnrom {
    fn read_prg(&self, ctx: &impl super::Context, addr: u16) -> u8 {
        self.ctrl.read_prg(ctx.rom(), addr)
    }

    fn write_prg(&mut self, ctx: &mut impl super::Context, addr: u16, data: u8) {
        for i in 0..8 {
            self.ctrl.map_chr(ctx.rom(), i, data as usize * 8 + i);
        }
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
