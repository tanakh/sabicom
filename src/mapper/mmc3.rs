use serde::{Deserialize, Serialize};

use crate::{
    consts::{LINES_PER_FRAME, PPU_CLOCK_PER_LINE, PRE_RENDER_LINE, SCREEN_RANGE},
    context::IrqSource,
    rom::Mirroring,
};

use bitvec::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct Mmc3 {
    cmd: u8,
    prg_swap: bool,
    chr_swap: bool,
    prg_bank: [u8; 2],
    chr_bank: [u8; 6],
    mirroring: Mirroring,
    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enable: bool,
    ppu_cycle: u64,
    ppu_line: u64,
    ppu_frame: u64,
    ppu_bus_addr: u16,
    ppu_a12_edge: bool,
}

impl Mmc3 {
    pub fn new(ctx: &mut impl super::Context) -> Self {
        let mirroring = ctx.rom().mirroring;
        let mut ret = Self {
            cmd: 0,
            prg_swap: false,
            chr_swap: false,
            prg_bank: [0, 1],
            chr_bank: [0; 6],
            mirroring,
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enable: false,
            ppu_cycle: 0,
            ppu_line: 0,
            ppu_frame: 0,
            ppu_bus_addr: 0,
            ppu_a12_edge: false,
        };
        ret.update(ctx);
        ret
    }

    fn update(&mut self, ctx: &mut impl super::Context) {
        let chr_swap = self.chr_swap as u32 * 4;
        for i in 0..2 {
            let bank = self.chr_bank[i] as u32;
            ctx.map_chr((i * 2) as u32 ^ chr_swap, bank & !1);
            ctx.map_chr((i * 2 + 1) as u32 ^ chr_swap, bank | 1);
        }
        for i in 2..6 {
            ctx.map_chr((i + 2) as u32 ^ chr_swap, self.chr_bank[i] as _);
        }

        let prg_pages = ctx.memory_ctrl().prg_pages();
        if !self.prg_swap {
            ctx.map_prg(0, self.prg_bank[0] as _);
            ctx.map_prg(1, self.prg_bank[1] as _);
            ctx.map_prg(2, prg_pages - 2);
            ctx.map_prg(3, prg_pages - 1);
        } else {
            ctx.map_prg(0, prg_pages - 2);
            ctx.map_prg(1, self.prg_bank[1] as _);
            ctx.map_prg(2, self.prg_bank[0] as _);
            ctx.map_prg(3, prg_pages - 1);
        }

        ctx.memory_ctrl_mut().set_mirroring(self.mirroring);
    }

    fn update_ppu_addr(&mut self, addr: u16) {
        if addr >= 0x2000 {
            return;
        }

        if self.ppu_bus_addr & 0x1000 == 0 && addr & 0x1000 != 0 {
            self.ppu_a12_edge = true;
        }

        self.ppu_bus_addr = addr;
    }
}

impl super::MapperTrait for Mmc3 {
    fn write_prg(&mut self, ctx: &mut impl super::Context, addr: u16, data: u8) {
        if addr & 0x8000 == 0 {
            ctx.write_prg(addr, data);
            return;
        }

        match addr & 0xE001 {
            0x8000 => {
                let v = data.view_bits::<Lsb0>();
                self.cmd = v[0..3].load();
                self.prg_swap = v[6];
                self.chr_swap = v[7];
            }
            0x8001 => {
                match self.cmd {
                    0..=5 => self.chr_bank[self.cmd as usize] = data,
                    6..=7 => self.prg_bank[self.cmd as usize - 6] = data,
                    _ => unreachable!(),
                }
                self.update(ctx);
            }

            0xA000 => {
                if self.mirroring != Mirroring::FourScreen {
                    self.mirroring = if data & 1 == 0 {
                        Mirroring::Vertical
                    } else {
                        Mirroring::Horizontal
                    };
                }
                self.update(ctx);
            }
            0xA001 => {
                let v = data.view_bits::<Lsb0>();
                log::info!("PRG RAM protect: enable: {}, write protect: {}", v[7], v[6]);
            }

            0xC000 => {
                log::trace!(
                    "MMC3 IRQ latch  : {data:3}, PPU frame={}, line={}, pixel={}",
                    self.ppu_frame,
                    self.ppu_line,
                    self.ppu_cycle
                );
                self.irq_latch = data
            }
            0xC001 => {
                log::trace!(
                    "MMC3 IRQ reload :      PPU frame={}, line={}, pixel={}",
                    self.ppu_frame,
                    self.ppu_line,
                    self.ppu_cycle
                );
                self.irq_counter = 0;
                self.irq_reload = true;
            }

            0xE000 => {
                log::trace!(
                    "MMC3 IRQ disable:      PPU frame={}, line={}, pixel={}",
                    self.ppu_frame,
                    self.ppu_line,
                    self.ppu_cycle
                );
                self.irq_enable = false;
                ctx.set_irq_source(IrqSource::Mapper, false);
            }
            0xE001 => {
                log::trace!(
                    "MMC3 IRQ enable :      PPU frame={}, line={}, pixel={}",
                    self.ppu_frame,
                    self.ppu_line,
                    self.ppu_cycle
                );
                self.irq_enable = true;
            }

            _ => unreachable!(),
        }
    }

    fn read_chr(&mut self, ctx: &mut impl super::Context, addr: u16) -> u8 {
        self.update_ppu_addr(addr);
        ctx.read_chr(addr)
    }

    fn write_chr(&mut self, ctx: &mut impl super::Context, addr: u16, data: u8) {
        self.update_ppu_addr(addr);
        ctx.write_chr(addr, data);
    }

    fn tick(&mut self, ctx: &mut impl super::Context) {
        if (self.ppu_line < SCREEN_RANGE.end as u64 || self.ppu_line == PRE_RENDER_LINE as u64)
            && self.ppu_cycle == 260
        {
            if self.ppu_a12_edge {
                let tmp = self.irq_counter;
                if self.irq_counter == 0 || self.irq_reload {
                    self.irq_counter = self.irq_latch;
                    self.irq_reload = false;
                } else {
                    self.irq_counter -= 1;
                }
                if (tmp > 0 || self.irq_reload) && self.irq_counter == 0 && self.irq_enable {
                    ctx.set_irq_source(IrqSource::Mapper, true);
                }
            }
            self.ppu_a12_edge = false;
        }

        self.ppu_cycle += 1;
        if self.ppu_cycle == PPU_CLOCK_PER_LINE {
            self.ppu_cycle = 0;
            self.ppu_line += 1;
            if self.ppu_line == LINES_PER_FRAME as u64 {
                self.ppu_line = 0;
                self.ppu_frame += 1;
            }
        }
    }
}
