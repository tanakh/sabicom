use crate::{
    apu::Apu,
    consts::*,
    cpu::{self, Cpu},
    mapper::{create_mapper, Mapper},
    memory::MemoryMap,
    ppu::{self, Ppu},
    rom::Rom,
    util::{clone_ref, wrap_ref, FrameBuffer, Input, Ref, Wire},
};

pub struct Nes {
    pub cpu: Cpu,
    ppu: Ref<Ppu>,
    apu: Ref<Apu>,
    pub mem: Ref<MemoryMap>,
    mapper: Ref<dyn Mapper>,
    rom: Ref<Rom>,
    frame_buf: FrameBuffer,
    audio_buf: Vec<i16>,
    counter: u64,
    wires: Wires,
}

struct Wires {
    nmi_wire: Wire<bool>,
    rst_wire: Wire<bool>,
    apu_frame_irq_wire: Wire<bool>,
    apu_dmc_irq_wire: Wire<bool>,
    mapper_irq_wire: Wire<bool>,
    cpu_irq_wire: Wire<bool>,
}

impl Wires {
    fn new() -> Self {
        Self {
            nmi_wire: Wire::new(false),
            rst_wire: Wire::new(false),
            apu_frame_irq_wire: Wire::new(false),
            apu_dmc_irq_wire: Wire::new(false),
            mapper_irq_wire: Wire::new(false),
            cpu_irq_wire: Wire::new(false),
        }
    }
    fn sync(&self) {
        let irq = self.apu_frame_irq_wire.get()
            || self.apu_dmc_irq_wire.get()
            || self.mapper_irq_wire.get();
        self.cpu_irq_wire.set(irq);
    }
}

pub struct State {}

impl Nes {
    pub fn new(rom: Rom, _sram: Option<Vec<u8>>) -> Self {
        let rom = wrap_ref(rom);
        let wires = Wires::new();

        let mapper = create_mapper(clone_ref(&rom), wires.mapper_irq_wire.clone());

        // FIXME: irq wire connect to or gate

        let ppu = wrap_ref(Ppu::new(
            clone_ref(&mapper),
            ppu::Wires {
                nmi: wires.nmi_wire.clone(),
            },
        ));
        let apu = wrap_ref(Apu::new(
            clone_ref(&mapper),
            wires.apu_frame_irq_wire.clone(),
            wires.apu_dmc_irq_wire.clone(),
        ));

        let mem = wrap_ref(MemoryMap::new(
            clone_ref(&ppu),
            clone_ref(&apu),
            clone_ref(&mapper),
        ));
        let cpu = Cpu::new(
            clone_ref(&mem),
            cpu::Wires {
                nmi: wires.nmi_wire.clone(),
                irq: wires.cpu_irq_wire.clone(),
                rst: wires.rst_wire.clone(),
            },
        );

        let frame_buf = FrameBuffer::new(SCREEN_WIDTH, SCREEN_HEIGHT);

        Self {
            rom,
            ppu,
            apu,
            cpu,
            mem,
            mapper,
            frame_buf,
            audio_buf: vec![],
            counter: 0,
            wires,
        }
    }

    pub fn reset(&mut self) {
        todo!("reset")
    }

    pub fn exec_frame(&mut self, input: &Input) {
        self.apu.borrow_mut().set_input(input);

        for _ in 0..PPU_CLOCK_PER_FRAME {
            self.counter += 1;
            if self.counter > PPU_CLOCK_PER_CPU_CLOCK {
                self.counter -= PPU_CLOCK_PER_CPU_CLOCK;

                self.wires.sync();
                self.cpu.tick();
                self.apu.borrow_mut().tick();
            }
            self.ppu.borrow_mut().tick();
            self.mapper.borrow_mut().tick();
        }

        self.frame_buf
            .buf
            .copy_from_slice(&self.ppu.borrow().frame_buf.buf);

        {
            let mut apu = self.apu.borrow_mut();
            self.audio_buf.resize(apu.audio_buf.len(), 0);
            for i in 0..self.audio_buf.len() {
                self.audio_buf[i] = apu.audio_buf[i];
            }
            apu.audio_buf.clear();
        }
    }

    pub fn get_frame_buf(&self) -> &FrameBuffer {
        &self.frame_buf
    }

    pub fn get_audio_buf(&self) -> &[i16] {
        &self.audio_buf
    }

    pub fn save_state(&self) -> State {
        todo!("save state")
    }

    pub fn load_state(&mut self, state: State) {
        todo!("load state")
    }
}
