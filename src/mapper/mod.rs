mod cnrom;
mod mmc1;
mod mmc3;
mod null;
mod unrom;

use crate::{
    rom::Rom,
    util::{wrap_ref, Ref, Wire},
};

pub trait Mapper {
    fn read_prg(&mut self, addr: u16) -> u8;
    fn write_prg(&mut self, addr: u16, data: u8);

    fn read_chr(&mut self, addr: u16) -> u8;
    fn write_chr(&mut self, addr: u16, data: u8);

    fn tick(&mut self) {}
}

pub fn create_mapper(rom: Ref<Rom>, irq_line: Wire<bool>) -> Ref<dyn Mapper> {
    let mapper_id = rom.borrow().mapper_id;

    match mapper_id {
        0 => wrap_ref(null::NullMapper::new(rom)),
        1 => wrap_ref(mmc1::Mmc1::new(rom)),
        2 => wrap_ref(unrom::Unrom::new(rom)),
        3 => wrap_ref(cnrom::Cnrom::new(rom)),
        4 => wrap_ref(mmc3::Mmc3::new(rom, irq_line)),
        _ => panic!("Unsupported mapper: {mapper_id}"),
    }
}
