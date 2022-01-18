use std::{cell::RefCell, rc::Rc};

use crate::{
    consts::*,
    cpu::Cpu,
    mapper::{create_mapper, Mapper},
    memory::MemoryMap,
    ppu::Ppu,
    rom::Rom,
};

type Ref<T> = Rc<RefCell<T>>;

fn wrap_ref<T>(v: T) -> Ref<T> {
    Rc::new(RefCell::new(v))
}

pub struct Nes {
    cpu: Cpu,
    ppu: Ref<Ppu>,
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
        let ppu = wrap_ref(Ppu::new());

        let mapper = create_mapper(Rc::clone(&rom));
        let mem = Rc::new(RefCell::new(MemoryMap::new(Rc::clone(&ppu), mapper)));
        let cpu = Cpu::new(Rc::clone(&mem));

        Self { rom, ppu, cpu, mem }
    }

    pub fn reset(&mut self) {
        todo!("reset")
    }

    pub fn exec_frame(&mut self) {
        for _ in 0..CLOCK_PER_FRAME {
            self.cpu.tick();
            self.ppu.borrow_mut().tick();
        }
    }

    pub fn save_state(&self) -> State {
        todo!("save state")
    }

    pub fn load_state(&mut self, state: State) {
        todo!("load state")
    }
}
