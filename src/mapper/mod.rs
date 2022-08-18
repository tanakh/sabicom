mod cnrom;
mod mmc1;
mod mmc3;
mod null;
mod unrom;

use ambassador::{delegatable_trait, Delegate};
use serde::{Deserialize, Serialize};

use crate::{context, nes::Error, util::trait_alias};

trait_alias!(pub trait Context = context::MemoryController + context::Rom + context::Interrupt);

#[delegatable_trait]
pub trait MapperTrait {
    fn read_prg(&self, ctx: &impl Context, addr: u16) -> u8 {
        ctx.read_prg(addr)
    }

    fn write_prg(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        ctx.write_prg(addr, data);
    }

    fn read_chr(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        ctx.read_chr(addr)
    }

    fn write_chr(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        ctx.write_chr(addr, data);
    }

    fn tick(&mut self, _ctx: &mut impl Context) {}
}

macro_rules! def_mapper {
    ($($id:expr => $constr:ident($ty:ty),)*) => {
        #[derive(Delegate, Serialize, Deserialize)]
        #[delegate(MapperTrait)]
        pub enum Mapper {
            $(
                $constr($ty),
            )*
        }

        pub fn create_mapper(ctx: &mut impl Context) -> Result<Mapper, Error> {
            let mapper_id = ctx.rom().mapper_id;
            Ok(match mapper_id {
                $(
                    $id => Mapper::$constr(<$ty>::new(ctx)),
                )*
                _ => Err(Error::UnsupportedMapper(mapper_id))?,
            })
        }
    }
}

def_mapper! {
    0 => NullMapper(null::NullMapper),
    1 => Mmc1(mmc1::Mmc1),
    2 => Unrom(unrom::Unrom),
    3 => Cnrom(cnrom::Cnrom),
    4 => Mmc3(mmc3::Mmc3),
}
