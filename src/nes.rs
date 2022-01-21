use crate::{
    apu::Apu,
    consts::*,
    cpu::{self, Cpu},
    mapper::create_mapper,
    memory::MemoryMap,
    ppu::{self, Ppu},
    rom::Rom,
    util::{clone_ref, wrap_ref, FrameBuffer, Input, Ref, Wire},
};

pub struct Nes {
    cpu: Cpu,
    ppu: Ref<Ppu>,
    apu: Ref<Apu>,
    pub mem: Ref<MemoryMap>,
    rom: Ref<Rom>,
    frame_buf: FrameBuffer,
}

pub struct State {}

impl Nes {
    pub fn new(rom: Rom, _sram: Option<Vec<u8>>) -> Self {
        let rom = wrap_ref(rom);
        let mapper = create_mapper(clone_ref(&rom));

        let nmi_wire = Wire::new(false);
        let irq_wire = Wire::new(false);
        let rst_wire = Wire::new(false);

        let ppu = wrap_ref(Ppu::new(
            clone_ref(&mapper),
            ppu::Wires {
                nmi: nmi_wire.clone(),
            },
        ));
        let apu = wrap_ref(Apu::new());

        let mem = wrap_ref(MemoryMap::new(clone_ref(&ppu), clone_ref(&apu), mapper));
        let cpu = Cpu::new(
            clone_ref(&mem),
            cpu::Wires {
                nmi: nmi_wire.clone(),
                irq: irq_wire.clone(),
                rst: rst_wire.clone(),
            },
        );

        let frame_buf = FrameBuffer::new(SCREEN_WIDTH, SCREEN_HEIGHT);

        Self {
            rom,
            ppu,
            apu,
            cpu,
            mem,
            frame_buf,
        }
    }

    pub fn reset(&mut self) {
        todo!("reset")
    }

    pub fn exec_frame(&mut self, input: &Input) {
        self.apu.borrow_mut().set_input(input);

        for _ in 0..CLOCK_PER_FRAME {
            self.cpu.tick();
            self.ppu.borrow_mut().tick();
        }

        self.frame_buf
            .buf
            .copy_from_slice(&self.ppu.borrow().frame_buf.buf);
    }

    pub fn get_frame_buf(&self) -> &FrameBuffer {
        &self.frame_buf
    }

    pub fn save_state(&self) -> State {
        todo!("save state")
    }

    pub fn load_state(&mut self, state: State) {
        todo!("load state")
    }
}
