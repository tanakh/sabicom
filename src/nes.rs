use std::{cell::RefCell, rc::Rc};

use crate::{
    cpu::Cpu,
    mapper::{create_mapper, Mapper},
    memory::MemoryMap,
    rom::Rom,
};

type Ref<T> = Rc<RefCell<T>>;

fn wrap_ref<T>(v: T) -> Ref<T> {
    Rc::new(RefCell::new(v))
}

pub struct Nes {
    cpu: Cpu,
    mem: Ref<MemoryMap>,
    rom: Ref<Rom>,
}

pub struct State {}

trait HelperTrait {
    fn wrap_in_refcell(self: Box<Self>) -> Rc<RefCell<dyn Mapper>>;
}

impl<T: Mapper + 'static> HelperTrait for T {
    fn wrap_in_refcell(self: Box<Self>) -> Rc<RefCell<dyn Mapper>> {
        Rc::new(RefCell::new(*self))
    }
}

impl Nes {
    pub fn new(rom: Rom, _sram: Option<Vec<u8>>) -> Self {
        let rom = wrap_ref(rom);

        let mapper = create_mapper(Rc::clone(&rom));
        let mem = Rc::new(RefCell::new(MemoryMap::new(mapper)));

        Self {
            rom,
            cpu: Cpu::new(Rc::clone(&mem)),
            mem,
        }
    }

    pub fn reset(&mut self) {
        todo!("reset")
    }

    pub fn exec_frame(&mut self) {
        const CLOCK_PER_LINE: u64 = 114;
        const LINES: usize = 262;

        for _ in 0..LINES {
            self.cpu.exec(CLOCK_PER_LINE);
        }
    }

    pub fn save_state(&self) -> State {
        todo!("save state")
    }

    pub fn load_state(&mut self, state: State) {
        todo!("load state")
    }
}
