use ambassador::{delegatable_trait, Delegate};
use serde::{Deserialize, Serialize};

use crate::{
    apu, cpu,
    mapper::{self, create_mapper},
    memory,
    nes::Error,
    ppu, rom,
};

#[delegatable_trait]
pub trait Cpu {
    fn reset_cpu(&mut self);
    fn tick_cpu(&mut self);
}

#[delegatable_trait]
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn read_pure(&self, addr: u16) -> Option<u8>;
    fn write(&mut self, addr: u16, data: u8);
    fn tick_bus(&mut self);
    fn cpu_stall(&mut self) -> u64;
}

#[delegatable_trait]
pub trait Ppu {
    fn ppu(&self) -> &ppu::Ppu;
    fn ppu_mut(&mut self) -> &mut ppu::Ppu;

    fn read_ppu(&mut self, addr: u16) -> u8;
    fn write_ppu(&mut self, addr: u16, data: u8);
    fn tick_ppu(&mut self);
}

#[delegatable_trait]
pub trait Apu {
    fn apu(&self) -> &apu::Apu;
    fn apu_mut(&mut self) -> &mut apu::Apu;

    fn read_apu(&mut self, addr: u16) -> u8;
    fn write_apu(&mut self, addr: u16, data: u8);
    fn tick_apu(&mut self);
}

#[delegatable_trait]
pub trait Mapper {
    fn read_prg(&self, addr: u16) -> u8;
    fn write_prg(&mut self, addr: u16, data: u8);
    fn read_chr(&self, addr: u16) -> u8;
    fn write_chr(&mut self, addr: u16, data: u8);
    fn prg_page(&self, page: u16) -> u16;
    fn tick_mapper(&mut self);
}

#[delegatable_trait]
pub trait Rom {
    fn rom(&self) -> &rom::Rom;
    fn rom_mut(&mut self) -> &mut rom::Rom;
}

pub enum IrqSource {
    ApuFrame = 0,
    ApuDmc = 1,
    Mapper = 2,
}

#[delegatable_trait]
pub trait Interrupt {
    fn rst(&mut self) -> bool;
    fn nmi(&mut self) -> bool;
    fn set_nmi(&mut self, nmi: bool);
    fn irq(&mut self) -> bool;
    fn irq_source(&self, source: IrqSource) -> bool;
    fn set_irq_source(&mut self, source: IrqSource, irq: bool);
}

#[delegatable_trait]
pub trait Timing {
    fn now(&self) -> u64;
    fn elapse(&mut self, elapsed: u64);
}

#[derive(Delegate, Serialize, Deserialize)]
#[delegate(Bus, target = "inner")]
#[delegate(Ppu, target = "inner")]
#[delegate(Apu, target = "inner")]
#[delegate(Mapper, target = "inner")]
#[delegate(Rom, target = "inner")]
#[delegate(Interrupt, target = "inner")]
#[delegate(Timing, target = "inner")]
pub struct Context {
    cpu: cpu::Cpu,
    inner: Inner,
}

impl Cpu for Context {
    fn reset_cpu(&mut self) {
        self.cpu.reset(&mut self.inner);
    }
    fn tick_cpu(&mut self) {
        self.cpu.tick(&mut self.inner);
    }
}

#[derive(Delegate, Serialize, Deserialize)]
#[delegate(Ppu, target = "inner")]
#[delegate(Apu, target = "inner")]
#[delegate(Mapper, target = "inner")]
#[delegate(Rom, target = "inner")]
#[delegate(Interrupt, target = "inner")]
#[delegate(Timing, target = "inner")]
struct Inner {
    mem: memory::MemoryMap,
    inner: Inner2,
}

impl Bus for Inner {
    fn read(&mut self, addr: u16) -> u8 {
        self.mem.read(&mut self.inner, addr)
    }

    fn read_pure(&self, addr: u16) -> Option<u8> {
        self.mem.read_pure(&self.inner, addr)
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.mem.write(&mut self.inner, addr, data);
    }

    fn tick_bus(&mut self) {
        self.mem.tick(&mut self.inner);
    }

    fn cpu_stall(&mut self) -> u64 {
        self.mem.cpu_stall()
    }
}

#[derive(Delegate, Serialize, Deserialize)]
#[delegate(Mapper, target = "inner")]
#[delegate(Rom, target = "inner")]
#[delegate(Interrupt, target = "inner")]
#[delegate(Timing, target = "inner")]
struct Inner2 {
    ppu: ppu::Ppu,
    apu: apu::Apu,
    inner: Inner3,
}

impl Ppu for Inner2 {
    fn ppu(&self) -> &ppu::Ppu {
        &self.ppu
    }
    fn ppu_mut(&mut self) -> &mut ppu::Ppu {
        &mut self.ppu
    }
    fn read_ppu(&mut self, addr: u16) -> u8 {
        self.ppu.read(&mut self.inner, addr)
    }
    fn write_ppu(&mut self, addr: u16, data: u8) {
        self.ppu.write(&mut self.inner, addr, data);
    }
    fn tick_ppu(&mut self) {
        self.ppu.tick(&mut self.inner);
    }
}

impl Apu for Inner2 {
    fn apu(&self) -> &apu::Apu {
        &self.apu
    }
    fn apu_mut(&mut self) -> &mut apu::Apu {
        &mut self.apu
    }
    fn read_apu(&mut self, addr: u16) -> u8 {
        self.apu.read(&mut self.inner, addr)
    }
    fn write_apu(&mut self, addr: u16, data: u8) {
        self.apu.write(&mut self.inner, addr, data);
    }
    fn tick_apu(&mut self) {
        self.apu.tick(&mut self.inner);
    }
}

#[derive(Delegate, Serialize, Deserialize)]
#[delegate(Rom, target = "inner")]
#[delegate(Interrupt, target = "inner")]
#[delegate(Timing, target = "inner")]
struct Inner3 {
    mapper: mapper::Mapper,
    inner: Inner4,
}

impl Mapper for Inner3 {
    fn read_prg(&self, addr: u16) -> u8 {
        use mapper::MapperTrait;
        self.mapper.read_prg(&self.inner, addr)
    }

    fn write_prg(&mut self, addr: u16, data: u8) {
        use mapper::MapperTrait;
        self.mapper.write_prg(&mut self.inner, addr, data);
    }

    fn read_chr(&self, addr: u16) -> u8 {
        use mapper::MapperTrait;
        self.mapper.read_chr(&self.inner, addr)
    }

    fn write_chr(&mut self, addr: u16, data: u8) {
        use mapper::MapperTrait;
        self.mapper.write_chr(&mut self.inner, addr, data);
    }

    fn prg_page(&self, page: u16) -> u16 {
        use mapper::MapperTrait;
        self.mapper.prg_page(page)
    }

    fn tick_mapper(&mut self) {
        use mapper::MapperTrait;
        self.mapper.tick(&mut self.inner)
    }
}

#[derive(Delegate, Serialize, Deserialize)]
#[delegate(Rom, target = "rom")]
#[delegate(Interrupt, target = "signales")]
struct Inner4 {
    #[serde(skip)]
    rom: rom::Rom,
    signales: Signales,
    now: u64,
}

impl Rom for rom::Rom {
    fn rom(&self) -> &rom::Rom {
        self
    }
    fn rom_mut(&mut self) -> &mut rom::Rom {
        self
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Signales {
    rst: bool,
    nmi: bool,
    irq_source: [bool; 3],
}

impl Interrupt for Signales {
    fn rst(&mut self) -> bool {
        self.rst
    }
    fn nmi(&mut self) -> bool {
        self.nmi
    }
    fn set_nmi(&mut self, nmi: bool) {
        self.nmi = nmi;
    }
    fn irq(&mut self) -> bool {
        self.irq_source.iter().any(|r| *r)
    }
    fn irq_source(&self, source: IrqSource) -> bool {
        self.irq_source[source as usize]
    }
    fn set_irq_source(&mut self, source: IrqSource, irq: bool) {
        self.irq_source[source as usize] = irq;
    }
}

impl Timing for Inner4 {
    fn now(&self) -> u64 {
        self.now
    }
    fn elapse(&mut self, elapsed: u64) {
        self.now += elapsed;
    }
}

impl Context {
    pub fn new(rom: rom::Rom, backup: Option<Vec<u8>>) -> Result<Context, Error> {
        let cpu = cpu::Cpu::default();
        let mem = memory::MemoryMap::new();
        let ppu = ppu::Ppu::new();
        let apu = apu::Apu::new();
        let mapper = create_mapper(&rom)?;
        let signales = Signales::default();

        Ok(Context {
            cpu,
            inner: Inner {
                mem,
                inner: Inner2 {
                    ppu,
                    apu,
                    inner: Inner3 {
                        mapper,
                        inner: Inner4 {
                            rom,
                            signales,
                            now: 0,
                        },
                    },
                },
            },
        })
    }
}
