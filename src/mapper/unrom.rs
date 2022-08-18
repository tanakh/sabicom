use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Unrom;

impl Unrom {
    pub fn new(ctx: &mut impl super::Context) -> Self {
        let prg_pages = ctx.memory_ctrl().prg_pages() as u32;
        ctx.map_prg(0, 0);
        ctx.map_prg(1, 1);
        ctx.map_prg(2, prg_pages - 2);
        ctx.map_prg(3, prg_pages - 1);
        Self
    }
}

impl super::MapperTrait for Unrom {
    fn write_prg(&mut self, ctx: &mut impl super::Context, _addr: u16, data: u8) {
        ctx.map_prg(0, data as u32 * 2);
        ctx.map_prg(1, data as u32 * 2 + 1);
    }
}
