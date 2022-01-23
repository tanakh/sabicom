mod cnrom;
mod mmc1;
mod null;
mod unrom;

use crate::{
    rom::Rom,
    util::{wrap_ref, Ref},
};

pub trait Mapper {
    fn read_prg(&mut self, addr: u16) -> u8;
    fn write_prg(&mut self, addr: u16, data: u8);

    fn read_chr(&mut self, addr: u16) -> u8;
    fn write_chr(&mut self, addr: u16, data: u8);
}

pub fn create_mapper(rom: Ref<Rom>) -> Ref<dyn Mapper> {
    let mapper_id = rom.borrow().mapper_id;

    match mapper_id {
        0 => wrap_ref(null::NullMapper::new(rom)),
        1 => wrap_ref(mmc1::Mmc1::new(rom)),
        2 => wrap_ref(unrom::Unrom::new(rom)),
        3 => wrap_ref(cnrom::Cnrom::new(rom)),
        _ => panic!("Unsupported mapper: {mapper_id}"),
    }
}
