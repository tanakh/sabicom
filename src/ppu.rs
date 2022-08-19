use bitvec::prelude::*;
use meru_interface::FrameBuffer;
use serde::{Deserialize, Serialize};

use crate::{consts::*, context, palette::NES_PALETTE, util::trait_alias};

trait_alias!(pub trait Context = context::Mapper + context::Interrupt);

#[derive(Serialize, Deserialize)]
pub struct Ppu {
    reg: Register,
    oam: Vec<u8>,
    counter: usize,
    line: usize,
    frame: u64,
    line_buf: Vec<u8>,
    sprite0_hit: Vec<bool>,

    #[serde(skip)]
    frame_buffer: FrameBuffer,
    render_graphics: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Register {
    buf: u8,
    vram_read_buf: u8,

    nmi_enable: bool,
    ppu_master: bool,
    sprite_size: bool,
    bg_pat_addr: bool,
    sprite_pat_addr: bool,
    ppu_addr_incr: bool,

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

impl Default for Ppu {
    fn default() -> Self {
        Self {
            reg: Register::new(),
            oam: vec![0x00; 256],
            counter: 0,
            line: 0,
            frame: 0,
            line_buf: vec![0x00; SCREEN_WIDTH],
            sprite0_hit: vec![false; SCREEN_WIDTH],
            frame_buffer: FrameBuffer::new(SCREEN_WIDTH, SCREEN_HEIGHT),
            render_graphics: true,
        }
    }
}

impl Ppu {
    pub fn frame_buffer(&self) -> &FrameBuffer {
        &self.frame_buffer
    }

    pub fn frame_buffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.frame_buffer
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    pub fn set_render_graphics(&mut self, render: bool) {
        self.render_graphics = render;
    }

    pub fn tick(&mut self, ctx: &mut impl Context) {
        // 1 PPU cycle for 1 pixel

        let screen_visible = self.reg.bg_visible || self.reg.sprite_visible;

        if self.counter == 0 {
            log::info!("line {} starts", self.line);

            if self.line == SCREEN_RANGE.start && screen_visible {
                self.reg.cur_addr = self.reg.tmp_addr;
            }

            if SCREEN_RANGE.contains(&self.line) && screen_visible {
                self.reg.cur_addr = (self.reg.cur_addr & 0xfbe0) | (self.reg.tmp_addr & 0x041f);
            }

            if SCREEN_RANGE.contains(&self.line) {
                self.render_line(ctx);

                if screen_visible {
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
        }

        if (self.line, self.counter) == (POST_RENDER_LINE + 1, 1) {
            log::info!("enter vblank");
            self.reg.vblank = true;
        }

        if (self.line, self.counter) == (PRE_RENDER_LINE, 1) {
            log::info!("leave vblank");
            self.reg.vblank = false;
            self.reg.sprite0_hit = false;
        }

        if screen_visible
            && (self.line < SCREEN_RANGE.end || self.line == PRE_RENDER_LINE)
            && self.counter == 256
        {
            let bg_pat_addr = if self.reg.bg_pat_addr { 0x1000 } else { 0 };
            let spr_pat_addr = if self.reg.sprite_pat_addr { 0x1000 } else { 0 };
            // FIXME: Dummy read for mapper that use CHR Address value
            let _ = read_pattern(ctx, bg_pat_addr);
            let _ = read_pattern(ctx, spr_pat_addr);
        }

        if screen_visible
            && SCREEN_RANGE.contains(&self.line)
            && self.counter < SCREEN_WIDTH
            && self.sprite0_hit[self.counter as usize]
        {
            self.reg.sprite0_hit = true;
        }

        self.counter += 1;

        if self.counter == PPU_CLOCK_PER_LINE as usize {
            self.counter = 0;
            self.line += 1;
            if self.line == LINES_PER_FRAME {
                self.line = 0;
                self.frame += 1;
            }
        }

        let nmi = !(self.reg.vblank && self.reg.nmi_enable);
        ctx.set_nmi(nmi);
    }

    pub fn render_line(&mut self, ctx: &mut impl Context) {
        let bg = read_palette(ctx, 0) & 0x3f;
        self.line_buf.fill(bg);
        self.sprite0_hit.fill(false);

        self.render_bg(ctx);
        self.render_spr(ctx);

        if self.reg.bg_clip || self.reg.sprite_clip {
            for i in 0..8 {
                assert!(!self.sprite0_hit[i]);
            }
        }

        for x in 0..SCREEN_WIDTH {
            *self.frame_buffer.pixel_mut(x, self.line) =
                NES_PALETTE[self.line_buf[x] as usize & 0x3f].clone();
        }
    }

    pub fn render_bg(&mut self, ctx: &mut impl Context) {
        let x_ofs = self.reg.scroll_x as usize;
        let y_ofs = (self.reg.cur_addr >> 12) & 7;
        let pat_addr = if self.reg.bg_pat_addr { 0x1000 } else { 0x0000 };
        let leftmost = if self.reg.bg_clip { 8 } else { 0 };

        let _ = read_pattern(ctx, pat_addr);

        if !self.reg.bg_visible {
            return;
        }

        let mut name_addr = self.reg.cur_addr & 0xfff;

        for i in 0..33 {
            let tile = read_nametable(ctx, name_addr) as u16 * 16;

            let b0 = read_pattern(ctx, pat_addr + tile + y_ofs);
            let b1 = read_pattern(ctx, pat_addr + tile + 8 + y_ofs);

            let name_addr_v = name_addr.view_bits::<Lsb0>();
            let tx = &name_addr_v[0..5];
            let ty = &name_addr_v[5..10];

            let attr_addr = bits![mut u16, Lsb0; 0; 16];
            attr_addr[10..12].copy_from_bitslice(&name_addr_v[10..12]);
            attr_addr[6..10].store(0b1111_u16);
            attr_addr[3..6].copy_from_bitslice(&ty[2..5]);
            attr_addr[0..3].copy_from_bitslice(&tx[2..5]);

            let aofs = tx[1] as usize * 2 + ty[1] as usize * 4;
            let attr = (read_nametable(ctx, attr_addr.load()) >> aofs) & 3;

            for lx in 0..8 {
                let x = (i * 8 + lx + 8 - x_ofs) as usize;
                if !(x >= 8 + leftmost && x < SCREEN_WIDTH + 8) {
                    continue;
                }

                let b = (b0 >> (7 - lx)) & 1 | ((b1 >> (7 - lx)) & 1) << 1;
                if b != 0 {
                    self.line_buf[x - 8] = 0x40 + read_palette(ctx, attr << 2 | b);
                }
            }

            if name_addr & 0x1f == 0x1f {
                name_addr = (name_addr & !0x1f) ^ 0x400;
            } else {
                name_addr += 1;
            }
        }
    }

    pub fn render_spr(&mut self, ctx: &mut impl Context) {
        if !self.reg.sprite_visible {
            return;
        }

        let spr_height = if self.reg.sprite_size { 16 } else { 8 };
        let pat_addr = if self.reg.sprite_pat_addr { 0x1000 } else { 0 };
        let leftmost = if self.reg.sprite_clip { 8 } else { 0 };

        for i in 0..64 {
            let r = &self.oam[i * 4..(i + 1) * 4];
            let spr_y = r[0] as usize + 1;

            if i == 0 {
                log::trace!("sprite {i}, y = {spr_y}, cur_line: {}", self.line);
            }

            if !(spr_y..spr_y + spr_height).contains(&self.line) {
                continue;
            }

            let tile_index = r[1] as u16;
            let spr_x = r[3] as usize;

            log::trace!("sprite {i}, x = {spr_x}, y = {spr_y}, tile = {tile_index}");

            let attr = r[2].view_bits::<Lsb0>();
            let upper = attr[0..2].load::<u8>() << 2;
            let is_bg = attr[5];
            let h_flip = !attr[6];
            let v_flip = attr[7];

            let y_ofs = if v_flip {
                spr_height - 1 - (self.line - spr_y)
            } else {
                self.line - spr_y
            };

            let tile_addr = if spr_height == 16 {
                (tile_index & !1) * 16
                    + (tile_index & 1) * 0x1000
                    + if y_ofs >= 8 { 16 } else { 0 }
                    + (y_ofs as u16 & 7)
            } else {
                pat_addr + tile_index * 16 + y_ofs as u16
            };

            let b0 = read_pattern(ctx, tile_addr);
            let b1 = read_pattern(ctx, tile_addr + 8);

            for lx in 0..8 {
                let x = spr_x + if h_flip { 7 - lx } else { lx };
                if !(x >= leftmost && x < SCREEN_WIDTH) {
                    continue;
                }

                let lo = (b0 >> lx) & 1 | ((b1 >> lx) & 1) << 1;
                if lo != 0 && self.line_buf[x] & 0x80 == 0 {
                    if i == 0 && x < 255 && self.line_buf[x] & 0x40 != 0 {
                        self.sprite0_hit[x] = true;
                    }
                    if !is_bg || self.line_buf[x] & 0x40 == 0 {
                        self.line_buf[x] = read_palette(ctx, 0x10 | upper | lo);
                    }
                    self.line_buf[x] |= 0x80;
                }
            }
        }
    }

    pub fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        let ret = match addr {
            2 => {
                // Status
                let ret = bits![mut u8, Lsb0; 0; 8];
                ret[0..5].store(self.reg.buf & 0x1f);
                ret.set(5, self.reg.sprite_over);
                ret.set(6, self.reg.sprite0_hit);
                ret.set(7, self.reg.vblank);

                self.reg.vblank = false;
                self.reg.toggle = false;

                log::info!(target: "ppureg", "[PPUSTATUS] -> ${ret:02X}");

                ret.load()
            }

            4 => {
                // OAM Data
                let ret = self.oam[self.reg.oam_addr as usize];
                let ret = if self.reg.oam_addr & 3 == 2 {
                    ret & 0xe3
                } else {
                    ret
                };

                log::info!(target: "ppureg", "[OAMDATA] -> ${ret:02X}",);

                ret
            }

            7 => {
                // Data
                let addr = self.reg.cur_addr & 0x3fff;

                let ret = if addr & 0x3f00 == 0x3f00 {
                    self.reg.vram_read_buf = ctx.read_chr_mapper(addr & !0x1000);
                    ctx.read_chr_mapper(addr)
                } else {
                    let ret = self.reg.vram_read_buf;
                    self.reg.vram_read_buf = ctx.read_chr_mapper(addr);
                    ret
                };

                let inc_addr = if self.reg.ppu_addr_incr { 32 } else { 1 };
                self.reg.cur_addr = self.reg.cur_addr.wrapping_add(inc_addr);

                log::info!(target: "ppureg", "[PPUDATA], CHR[${addr:04X}] -> ${ret:02X}");

                ret
            }

            _ => {
                log::info!("Read from invalid PPU register: [{addr}]");
                self.reg.buf
            }
        };

        self.reg.buf = ret;
        ret
    }

    pub fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        self.reg.buf = data;

        match addr {
            0 => {
                // Controller
                let data = data.view_bits::<Lsb0>();

                log::info!(
                    target: "ppureg::PPUCTRL",
                    "= b{data:08b}: nmi={nmi}, ppu={ppu}, spr={sprite_size}, bgpat=${bg_pat_addr:04X}, sprpat=${sprite_pat_addr:04X}, addrinc={ppu_addr_incr}, nt_addr=${base_nametable_addr:04X}",
                    nmi = if data[7] { "t" } else { "f" },
                    ppu = if data[6] { "t" } else { "f" },
                    sprite_size = if data[5] { "8x16" } else { "8x8" },
                    bg_pat_addr = if data[4] { 0x0000 } else { 0x1000 },
                    sprite_pat_addr =  if data[3] { 0x0000 } else { 0x1000 },
                    ppu_addr_incr =  if data[2] { 32 } else { 1 },
                    base_nametable_addr = 0x2000 + data[0..2].load_le::<u16>() * 0x400,
                );

                self.reg.nmi_enable = data[7];
                self.reg.ppu_master = data[6];
                self.reg.sprite_size = data[5];
                self.reg.bg_pat_addr = data[4];
                self.reg.sprite_pat_addr = data[3];
                self.reg.ppu_addr_incr = data[2];

                self.reg.tmp_addr.view_bits_mut::<Lsb0>()[10..12].store(data[0..2].load::<u16>());
            }

            1 => {
                // Mask
                let data = data.view_bits::<Lsb0>();

                log::info!(target: "ppureg::PPUMASK", "= b{data:08b}: bgcol={r}{g}{b}, spr_vis={sprite_visible}, bg_vis={bg_visible}, spr_clip={sprite_clip}, bg_clip={bg_clip}, greyscale={greyscale}",
                    r = if data[5] { "R" } else { "-" },
                    g = if data[6] { "G" } else { "-" },
                    b = if data[7] { "B" } else { "-" },
                    sprite_visible = if data[4] { "t" } else { "f" },
                    bg_visible = if data[3] { "t" } else { "f" },
                    sprite_clip = if data[2] { "t" } else { "f" },
                    bg_clip = if data[1] { "t" } else { "f" },
                    greyscale = if data[0] { "t" } else { "f" },
                );

                self.reg.bg_color = data[5..8].load_le();
                self.reg.sprite_visible = data[4];
                self.reg.bg_visible = data[3];
                self.reg.sprite_clip = !data[2];
                self.reg.bg_clip = !data[1];
                self.reg.color_display = data[0];
            }
            2 => {
                // Status
                log::warn!("Write to $2002 = {data:02X}");
            }
            3 => {
                // OAM address
                log::info!(target: "ppureg::OAMADDR", "= ${data:02X}");

                self.reg.oam_addr = data;
            }
            4 => {
                // OAM data
                log::info!(target: "ppureg::OAMDATA", "= ${data:02X}: OAM[${oam_addr:02X}] = ${data:02X}",
                    oam_addr = self.reg.oam_addr);

                self.oam[self.reg.oam_addr as usize] = data;
                self.reg.oam_addr = self.reg.oam_addr.wrapping_add(1);
            }
            5 => {
                // Scroll
                log::info!(target: "ppureg::PPUSCROLL", "= ${data:02X}");

                let data = data.view_bits::<Lsb0>();

                if !self.reg.toggle {
                    self.reg.tmp_addr = (self.reg.tmp_addr & 0x7fe0) | data[3..8].load_le::<u16>();
                    self.reg.scroll_x = data[0..3].load_le();
                } else {
                    self.reg.tmp_addr = (self.reg.tmp_addr & 0x0c1f)
                        | data[3..8].load_le::<u16>() << 5
                        | data[0..3].load_le::<u16>() << 12;
                }
                self.reg.toggle = !self.reg.toggle;
            }
            6 => {
                // Address
                log::info!(target: "ppureg::PPUADDR", "= ${data:02X}");

                let data = data.view_bits::<Lsb0>();

                if !self.reg.toggle {
                    self.reg.tmp_addr =
                        (self.reg.tmp_addr & 0x00ff) | data[0..6].load_be::<u16>() << 8;
                } else {
                    self.reg.tmp_addr = (self.reg.tmp_addr & 0x7f00) | data.load_be::<u16>();
                    self.reg.cur_addr = self.reg.tmp_addr;
                }
                self.reg.toggle = !self.reg.toggle;
            }
            7 => {
                // Data
                let addr = self.reg.cur_addr & 0x3fff;

                log::info!(target: "ppureg::PPUDATA", "= ${data:02X}, CHR[${addr:04X}] <- ${data:02X}");

                ctx.write_chr_mapper(addr, data);

                let inc_addr = if self.reg.ppu_addr_incr { 32 } else { 1 };
                self.reg.cur_addr = self.reg.cur_addr.wrapping_add(inc_addr);
            }
            _ => unreachable!(),
        }
    }
}

fn read_nametable(ctx: &mut impl Context, addr: u16) -> u8 {
    ctx.read_chr_mapper(0x2000 + addr)
}

fn read_pattern(ctx: &mut impl Context, addr: u16) -> u8 {
    ctx.read_chr_mapper(addr)
}

fn read_palette(ctx: &mut impl Context, index: u8) -> u8 {
    ctx.read_chr_mapper(0x3f00 + index as u16)
}
