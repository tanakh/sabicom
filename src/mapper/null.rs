use serde::{Deserialize, Serialize};

use crate::{memory::MemoryController, rom::Rom};

#[derive(Serialize, Deserialize)]
pub struct NullMapper {
    ctrl: MemoryController,
}

impl NullMapper {
    pub fn new(rom: &Rom) -> Self {
        Self {
            ctrl: MemoryController::new(rom),
        }
    }
}

impl super::MapperTrait for NullMapper {
    fn read_prg(&self, ctx: &impl super::Context, addr: u16) -> u8 {
        self.ctrl.read_prg(ctx.rom(), addr)
    }

    fn write_prg(&mut self, ctx: &mut impl super::Context, addr: u16, data: u8) {
        self.ctrl.write_prg(ctx.rom(), addr, data);
    }

    fn read_chr(&mut self, ctx: &mut impl super::Context, addr: u16) -> u8 {
        self.ctrl.read_chr(ctx.rom(), addr)
    }

    fn write_chr(&mut self, ctx: &mut impl super::Context, addr: u16, data: u8) {
        self.ctrl.write_chr(ctx.rom(), addr, data);
    }
}
