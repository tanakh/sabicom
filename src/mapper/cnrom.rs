use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Cnrom;

impl Cnrom {
    pub fn new(ctx: &mut impl super::Context) -> Self {
        for i in 0..4 {
            ctx.map_prg(i, i);
        }
        for i in 0..8 {
            ctx.map_chr(i, i);
        }
        Self
    }
}

impl super::MapperTrait for Cnrom {
    fn write_prg(&mut self, ctx: &mut impl super::Context, _addr: u16, data: u8) {
        for i in 0..8 {
            ctx.map_chr(i, data as u32 * 8 + i);
        }
    }
}
