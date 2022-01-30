use crate::{
    consts::{LINES_PER_FRAME, PPU_CLOCK_PER_LINE, PRE_RENDER_LINE, SCREEN_RANGE},
    memory::MemoryController,
    rom::{Mirroring, Rom},
    util::{Ref, Wire},
};

use bitvec::prelude::*;

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
    irq_line: Wire<bool>,
    ctrl: MemoryController,
}

impl Mmc3 {
    pub fn new(rom: Ref<Rom>, irq_line: Wire<bool>) -> Self {
        let mirroring = rom.borrow().mirroring;
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
            irq_line,
            ctrl: MemoryController::new(rom),
        };
        ret.update();
        ret
    }

    fn update(&mut self) {
        let chr_swap = self.chr_swap as usize * 4;
        for i in 0..2 {
            let bank = self.chr_bank[i] as usize;
            self.ctrl.map_chr((i * 2 + 0) ^ chr_swap, bank & !1);
            self.ctrl.map_chr((i * 2 + 1) ^ chr_swap, bank | 1);
        }
        for i in 2..6 {
            self.ctrl.map_chr((i + 2) ^ chr_swap, self.chr_bank[i] as _);
        }

        let prg_pages = self.ctrl.prg_pages();
        if !self.prg_swap {
            self.ctrl.map_prg(0, self.prg_bank[0] as _);
            self.ctrl.map_prg(1, self.prg_bank[1] as _);
            self.ctrl.map_prg(2, prg_pages - 2);
            self.ctrl.map_prg(3, prg_pages - 1);
        } else {
            self.ctrl.map_prg(0, prg_pages - 2);
            self.ctrl.map_prg(1, self.prg_bank[1] as _);
            self.ctrl.map_prg(2, self.prg_bank[0] as _);
            self.ctrl.map_prg(3, prg_pages - 1);
        }

        self.ctrl.set_mirroring(self.mirroring);
    }
}

impl super::Mapper for Mmc3 {
    fn read_prg(&mut self, addr: u16) -> u8 {
        self.ctrl.read_prg(addr)
    }

    fn read_chr(&mut self, addr: u16) -> u8 {
        self.update_ppu_addr(addr);
        self.ctrl.read_chr(addr)
    }

    fn write_prg(&mut self, addr: u16, data: u8) {
        if addr & 0x8000 == 0 {
            return self.ctrl.write_prg(addr, data);
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
                self.update()
            }

            0xA000 => {
                if self.mirroring != Mirroring::FourScreen {
                    self.mirroring = if data & 1 == 0 {
                        Mirroring::Vertical
                    } else {
                        Mirroring::Horizontal
                    };
                }
                self.update()
            }
            0xA001 => {
                let v = data.view_bits::<Lsb0>();
                log::warn!("PRG RAM protect: enable: {}, write protect: {}", v[7], v[6]);
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
                self.irq_line.set(false);
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

    fn write_chr(&mut self, addr: u16, data: u8) {
        self.update_ppu_addr(addr);
        self.ctrl.write_chr(addr, data);
    }

    fn tick(&mut self) {
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
                    self.irq_line.set(true);
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

    fn get_prg_page(&self, page: usize) -> usize {
        self.ctrl.get_prg_page(page)
    }
}

impl Mmc3 {
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
