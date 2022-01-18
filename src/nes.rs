use crate::{
    apu::Apu,
    consts::*,
    cpu::Cpu,
    mapper::{create_mapper, Mapper},
    memory::MemoryMap,
    ppu::Ppu,
    rom::Rom,
    util::{clone_ref, wrap_ref, Ref},
};

pub struct Nes {
    cpu: Cpu,
    ppu: Ref<Ppu>,
    apu: Ref<Apu>,
    mem: Ref<MemoryMap>,
    rom: Ref<Rom>,
}

pub struct State {}

impl Nes {
    pub fn new(rom: Rom, _sram: Option<Vec<u8>>) -> Self {
        let rom = wrap_ref(rom);
        let ppu = wrap_ref(Ppu::new());
        let apu = wrap_ref(Apu::new());

        let mapper = create_mapper(clone_ref(&rom));
        let mem = wrap_ref(MemoryMap::new(clone_ref(&ppu), clone_ref(&apu), mapper));
        let cpu = Cpu::new(clone_ref(&mem));

        Self {
            rom,
            ppu,
            apu,
            cpu,
            mem,
        }
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
