use crate::consts::*;

pub struct Ppu {
    reg: Register,
    oam: Vec<u8>,
    palette: Vec<u8>,
    line: usize,
    counter: u64,
}

#[derive(Default)]
struct Register {
    nmi_enable: bool,
    ppu_master: bool,
    sprite_size: bool,
    bg_pat_adr: bool,
    sprite_pat_adr: bool,
    ppu_addr_incr: bool,
    base_nametable_addr: u8,

    bg_color: u8,
    sprite_visible: bool,
    bg_visible: bool,
    sprite_clip: bool,
    bg_clip: bool,
    color_display: bool,

    oam_addr: u8,

    toggle: bool,
    scroll_x: u8,
    tmp_scroll: u16,
    scroll: u16,

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
    pub fn new() -> Self {
        Self {
            reg: Register::new(),
            oam: vec![0x00; 256],
            palette: vec![0x00; 64],
            counter: 0,
            line: 0,
        }
    }

    pub fn tick(&mut self) {
        self.counter += 1;

        if self.counter >= CLOCK_PER_LINE {
            self.counter -= CLOCK_PER_LINE;

            self.line += 1;
            if self.line >= TOTAL_LINES {
                self.line = 0;
            }

            if self.line == POST_RENDER_LINE {
                self.reg.vblank = true;
            }

            if self.line == PRE_RENDER_LINE {
                self.reg.vblank = false;
                self.reg.sprite0_hit = false;
            }
        }
    }

    pub fn read_reg(&mut self, addr: u16) -> u8 {
        match addr {
            2 => {
                // PPU Status Register (R)
                let mut ret = 0;
                ret |= if self.reg.vblank { 0x80 } else { 0 };
                ret |= if self.reg.sprite0_hit { 0x40 } else { 0 };
                ret |= if self.reg.sprite_over { 0x20 } else { 0 };
                // FIXME: Least significant bits previously written into a PPU register
                self.reg.vblank = false;
                self.reg.toggle = false;
                ret
            }

            7 => {
                // VRAM I/O Register (RW)
                todo!("Read from $2007");
            }

            _ => {
                log::warn!("Read from PPU register: {addr}");
                0
            }
        }
    }

    pub fn write_reg(&mut self, addr: u16, val: u8) {
        match addr {
            0 => {
                // PPU Control Register #1 (W)
                self.reg.nmi_enable = val & 0x80 != 0;
                self.reg.ppu_master = val & 0x40 != 0;
                self.reg.sprite_size = val & 0x20 != 0;
                self.reg.bg_pat_adr = val & 0x10 != 0;
                self.reg.sprite_pat_adr = val & 0x08 != 0;
                self.reg.ppu_addr_incr = val & 0x04 != 0;
                self.reg.base_nametable_addr = val & 0x03;
            }

            1 => {
                // PPU Control Register #2 (W)
                self.reg.bg_color = val >> 5;
                self.reg.sprite_visible = val & 0x10 != 0;
                self.reg.bg_visible = val & 0x08 != 0;
                self.reg.sprite_clip = val & 0x04 != 0;
                self.reg.bg_clip = val & 0x02 != 0;
                self.reg.color_display = val & 0x01 != 0;
            }
            2 => {
                // PPU Status Register (R)
                log::warn!("Write to $2002 = {val:02X}");
            }
            3 => {
                // SPR-RAM Address Register (W)
                self.reg.oam_addr = val;
            }
            4 => {
                // SPR-RAM I/O Register (W)
                self.oam[self.reg.oam_addr as usize] = val;
                self.reg.oam_addr = self.reg.oam_addr.wrapping_add(1);
            }
            5 => {
                // VRAM Address Register #1 (W2)
                if !self.reg.toggle {
                    self.reg.tmp_scroll = (self.reg.tmp_scroll & 0x7fe0) | (val as u16 >> 3);
                    self.reg.scroll_x = val & 0x07;
                } else {
                    self.reg.tmp_scroll = (self.reg.tmp_scroll & 0x0c1f)
                        | ((val as u16 & 0xF8) << 2)
                        | ((val as u16 & 0x07) << 12);
                }
                self.reg.toggle = !self.reg.toggle;
            }
            6 => {
                // VRAM Address Register #2 (W2)
                if !self.reg.toggle {
                    self.reg.tmp_scroll =
                        (self.reg.tmp_scroll & 0x00ff) | ((val as u16 & 0x7f) << 8);
                } else {
                    self.reg.tmp_scroll = (self.reg.tmp_scroll & 0x7f00) | val as u16;
                    self.reg.scroll = self.reg.tmp_scroll;
                }
                self.reg.toggle = !self.reg.toggle;
            }
            7 => {
                // VRAM I/O Register (RW)

                if self.reg.scroll & 0x3f00 != 0x3f00 {
                    let addr = self.reg.scroll & 0x1f;
                    log::warn!("Write to CHR-ROM (${addr:04X}) = {val:02X}");
                } else {
                    let addr = self.reg.scroll & 0x1f;
                    let val = val & 0x3f;
                    self.palette[addr as usize] = val;
                    if addr & 3 == 0 {
                        self.palette[(addr ^ 0x10) as usize] = val;
                    }
                }

                self.reg.scroll =
                    self.reg
                        .scroll
                        .wrapping_add(if self.reg.ppu_addr_incr { 32 } else { 1 });
            }
            _ => unreachable!(),
        }
    }
}
