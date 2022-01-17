mod null;

use crate::rom::Rom;
use std::{cell::RefCell, rc::Rc};

pub trait Mapper {
    fn read_u8(&self, addr: u16) -> u8;
    fn write_u8(&mut self, addr: u16, val: u8);
}

pub fn create_mapper(rom: Rc<RefCell<Rom>>) -> Rc<RefCell<dyn Mapper>> {
    let mapper_id = rom.borrow().mapper_id;

    match mapper_id {
        0 => Rc::new(RefCell::new(null::NullMapper::new(rom))),
        _ => panic!("Unsupported mapper: {mapper_id}"),
    }
}
