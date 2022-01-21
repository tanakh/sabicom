use crate::{
    consts::*,
    mapper::Mapper,
    palette::NES_PALETTE,
    util::{FrameBuffer, Ref, Wire},
};

pub struct Ppu {
    reg: Register,
    oam: Vec<u8>,
    line: usize,
    counter: u64,
    mapper: Ref<dyn Mapper>,
    wires: Wires,
    pub frame_buf: FrameBuffer,
}

pub struct Wires {
    pub nmi: Wire<bool>,
}

#[derive(Default)]
struct Register {
    nmi_enable: bool,
    ppu_master: bool,
    sprite_size: bool,
    bg_pat_addr: bool,
    sprite_pat_addr: bool,
    ppu_addr_incr: bool,

    // base_nametable_addr: u8,
    bg_color: u8,
    sprite_visible: bool,
    bg_visible: bool,
    sprite_clip: bool,
    bg_clip: bool,
    color_display: bool,

    oam_addr: u8,

    toggle: bool,
    scroll_x: u8,
    tmp_addr: u16,
    cur_addr: u16,

    vblank: bool,
    sprite0_hit: bool,
    sprite_over: bool,
}

impl Register {
    fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl Ppu {
    pub fn new(mapper: Ref<dyn Mapper>, wires: Wires) -> Self {
        Self {
            reg: Register::new(),
            oam: vec![0x00; 256],
            counter: 0,
            line: 0,
            mapper,
            wires,
            frame_buf: FrameBuffer::new(SCREEN_WIDTH, SCREEN_HEIGHT),
        }
    }

    pub fn tick(&mut self) {
        self.counter += 1;

        if self.counter >= CLOCK_PER_LINE {
            self.counter -= CLOCK_PER_LINE;

            if SCREEN_RANGE.contains(&self.line) {
                self.render_line();

                if self.reg.bg_visible || self.reg.sprite_visible {
                    if (self.reg.cur_addr >> 12) & 7 == 7 {
                        self.reg.cur_addr &= !0x7000;
                        if ((self.reg.cur_addr >> 5) & 0x1f) == 29 {
                            self.reg.cur_addr = (self.reg.cur_addr & !0x03e0) ^ 0x800;
                        } else if (self.reg.cur_addr >> 5) & 0x1f == 0x1f {
                            self.reg.cur_addr &= !0x03e0;
                        } else {
                            self.reg.cur_addr += 0x20;
                        }
                    } else {
                        self.reg.cur_addr += 0x1000;
                    }
                }
            }

            self.line += 1;
            if self.line >= TOTAL_LINES {
                self.line = 0;
            }

            log::info!("line {} starts", self.line);

            if self.line == POST_RENDER_LINE + 1 {
                log::info!("enter vblank");
                self.reg.vblank = true;
            }

            if self.line == PRE_RENDER_LINE {
                log::info!("leave vblank");
                self.reg.vblank = false;
                self.reg.sprite0_hit = false;
            }

            if SCREEN_RANGE.contains(&self.line) && (self.reg.bg_visible || self.reg.sprite_visible)
            {
                self.reg.cur_addr = (self.reg.cur_addr & 0xfbe0) | (self.reg.tmp_addr & 0x041f);
            }
        }

        let nmi_line = !(self.reg.vblank && self.reg.nmi_enable);
        self.wires.nmi.set(nmi_line);
    }

    pub fn render_line(&mut self) {
        let y = self.line;

        let pal0 = self.read_palette(0);
        let mut buf = vec![pal0; SCREEN_WIDTH];

        if self.reg.bg_visible {
            self.render_bg(&mut buf);
        }
        if self.reg.sprite_visible {
            self.render_spr(&mut buf);
        }

        for x in 0..SCREEN_WIDTH {
            self.frame_buf
                .set(x, y, NES_PALETTE[buf[x] as usize & 0x3f]);
        }
    }

    pub fn render_bg(&mut self, buf: &mut [u8]) {
        let x_ofs = self.reg.scroll_x as usize;
        let y_ofs = (self.reg.cur_addr >> 12) & 7;
        let pat_addr = if self.reg.bg_pat_addr { 0x1000 } else { 0x0000 };

        let mut name_addr = self.reg.cur_addr & 0xfff;

        for i in 0..33 {
            let tile = self.read_nametable(name_addr);
            let l = self.read_pattern(pat_addr + (tile as u16 * 16) + y_ofs);
            let h = self.read_pattern(pat_addr + (tile as u16 * 16) + y_ofs + 8);

            let tx = name_addr & 0x1f;
            let ty = (name_addr >> 5) & 0x1f;
            let attr_addr = (name_addr & 0xC00) + 0x3C0 + ((ty & !3) << 1) + (tx >> 2);
            let aofs = (if (ty & 2) == 0 { 0 } else { 4 }) + (if (tx & 2) == 0 { 0 } else { 2 });
            let attr = ((self.read_nametable(attr_addr) >> aofs) & 3) << 2;

            for lx in 0..8 {
                let x = (i * 8 + lx + 8 - x_ofs) as usize;
                if !(x >= 8 && x < SCREEN_WIDTH + 8) {
                    continue;
                }
                let b = (l >> (7 - lx)) & 1 | ((h >> (7 - lx)) & 1) << 1;
                buf[x - 8] = 0x40 + self.read_palette(b | attr);
            }

            if (name_addr & 0x1f) == 0x1f {
                name_addr = (name_addr & !0x1f) ^ 0x400;
            } else {
                name_addr += 1;
            }
        }
    }

    pub fn render_spr(&mut self, buf: &mut [u8]) {
        log::warn!("render sprite");
    }

    fn read_nametable(&self, addr: u16) -> u8 {
        self.mapper.borrow_mut().read_chr(0x2000 + addr)
    }

    fn read_pattern(&self, addr: u16) -> u8 {
        self.mapper.borrow_mut().read_chr(addr)
    }

    fn read_palette(&self, index: u8) -> u8 {
        self.mapper.borrow_mut().read_chr(0x3f00 + index as u16)
    }

    pub fn read_reg(&mut self, addr: u16) -> u8 {
        match addr {
            2 => {
                // Status
                let mut ret = 0;
                ret |= if self.reg.vblank { 0x80 } else { 0 };
                ret |= if self.reg.sprite0_hit { 0x40 } else { 0 };
                ret |= if self.reg.sprite_over { 0x20 } else { 0 };

                // FIXME: Least significant bits previously written into a PPU register
                self.reg.vblank = false;
                self.reg.toggle = false;

                log::info!(target: "ppureg", "[PPUSTATUS] -> ${:02X}", ret);

                ret
            }

            7 => {
                // Data
                let addr = self.reg.cur_addr & 0x3fff;

                let ret = self.mapper.borrow_mut().read_chr(addr);

                let inc_addr = if self.reg.ppu_addr_incr { 32 } else { 1 };
                self.reg.cur_addr = self.reg.cur_addr.wrapping_add(inc_addr);

                log::info!(target: "ppureg", "[PPUDATA], CHR[${addr:04X}] -> ${ret:02X}");

                ret
            }

            _ => {
                log::info!("Read from invalid PPU register: [{addr}]");
                0
            }
        }
    }

    pub fn write_reg(&mut self, addr: u16, data: u8) {
        match addr {
            0 => {
                // Controller
                log::info!(
                    target: "ppureg",
                    "[PPUCTRL] = b{data:08b}: nmi={nmi}, ppu={ppu}, spr={sprite_size}, bgpat=${bg_pat_addr:04X}, sprpat=${sprite_pat_addr:04X}, addrinc={ppu_addr_incr}, nt_addr=${base_nametable_addr:04X}",
                    nmi = if data & 0x80 != 0 { "t" } else { "f" },
                    ppu = if data & 0x40 != 0 { "t" } else { "f" },
                    sprite_size = if data & 0x20 != 0 { "8x16" } else { "8x8" },
                    bg_pat_addr = if data & 0x10 != 0 { 0x0000 } else { 0x1000 },
                    sprite_pat_addr =  if data & 0x08 != 0 { 0x0000 } else { 0x1000 },
                    ppu_addr_incr =  if data & 0x04 != 0 { 32 } else { 1 },
                    base_nametable_addr = 0x2000 + (data as u16 & 3) * 0x400,
                );

                self.reg.nmi_enable = data & 0x80 != 0;
                self.reg.ppu_master = data & 0x40 != 0;
                self.reg.sprite_size = data & 0x20 != 0;
                self.reg.bg_pat_addr = data & 0x10 != 0;
                self.reg.sprite_pat_addr = data & 0x08 != 0;
                self.reg.ppu_addr_incr = data & 0x04 != 0;

                self.reg.tmp_addr = (self.reg.tmp_addr & 0x73FF) | ((data as u16 & 3) << 10);
            }

            1 => {
                // Mask
                log::info!(target: "ppureg", "[PPUMASK] = b{data:08b}: bgcol={r}{g}{b}, spr_vis={sprite_visible}, bg_vis={bg_visible}, spr_clip={sprite_clip}, bg_clip={bg_clip}, greyscale={greyscale}",
                    r = if data & 0x20 != 0 { "R" } else { "-" },
                    g = if data & 0x40 != 0 { "G" } else { "-" },
                    b = if data & 0x80 != 0 { "B" } else { "-" },
                    sprite_visible = if data & 0x10 != 0 { "t" } else { "f" },
                    bg_visible = if data & 0x08 != 0 { "t" } else { "f" },
                    sprite_clip = if data & 0x04 != 0 { "f" } else { "t" },
                    bg_clip = if data & 0x02 != 0 { "f" } else { "t" },
                    greyscale = if data & 0x01 != 0 { "t" } else { "f" },
                );

                self.reg.bg_color = data >> 5;
                self.reg.sprite_visible = data & 0x10 != 0;
                self.reg.bg_visible = data & 0x08 != 0;
                self.reg.sprite_clip = data & 0x04 != 0;
                self.reg.bg_clip = data & 0x02 != 0;
                self.reg.color_display = data & 0x01 != 0;
            }
            2 => {
                // Status
                log::warn!("Write to $2002 = {data:02X}");
            }
            3 => {
                // OAM address
                log::info!(target: "ppureg", "[OAMADDR] <- ${data:02X}");

                self.reg.oam_addr = data;
            }
            4 => {
                // OAM data
                log::info!(target: "ppureg", "[OAMDATA] <- ${data:02X}: OAM[${oam_addr:02X}] = ${data:02X}",
                    oam_addr = self.reg.oam_addr);

                self.oam[self.reg.oam_addr as usize] = data;
                self.reg.oam_addr = self.reg.oam_addr.wrapping_add(1);
            }
            5 => {
                // Scroll
                log::info!(target: "ppureg", "[PPUSCROLL] <- ${data:02X}");

                if !self.reg.toggle {
                    self.reg.tmp_addr = (self.reg.tmp_addr & 0x7fe0) | (data as u16 >> 3);
                    self.reg.scroll_x = data & 0x07;
                } else {
                    self.reg.tmp_addr = (self.reg.tmp_addr & 0x0c1f)
                        | (data as u16 & 0xF8) << 5
                        | (data as u16 & 0x07) << 12;
                }
                self.reg.toggle = !self.reg.toggle;
            }
            6 => {
                // Address
                log::info!(target: "ppureg", "[PPUADDR] <- ${data:02X}");

                if !self.reg.toggle {
                    self.reg.tmp_addr = (self.reg.tmp_addr & 0x00ff) | ((data as u16 & 0x3f) << 8);
                } else {
                    self.reg.tmp_addr = (self.reg.tmp_addr & 0x7f00) | data as u16;
                    self.reg.cur_addr = self.reg.tmp_addr;
                }
                self.reg.toggle = !self.reg.toggle;
            }
            7 => {
                // Data
                let addr = self.reg.cur_addr & 0x3fff;

                log::info!(target: "ppureg", "[PPUDATA] <- ${data:02X}, CHR[${addr:04X}] <- ${data:02X}");

                self.mapper.borrow_mut().write_chr(addr, data);

                let inc_addr = if self.reg.ppu_addr_incr { 32 } else { 1 };
                self.reg.cur_addr = self.reg.cur_addr.wrapping_add(inc_addr);
            }
            _ => unreachable!(),
        }
    }
}
