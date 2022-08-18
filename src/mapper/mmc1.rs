use serde::{Deserialize, Serialize};

use crate::rom::Mirroring;

#[derive(Serialize, Deserialize)]
pub struct Mmc1 {
    prg_rom_bank_mode: PrgRomBankMode,
    chr_rom_bank_mode: ChrRomBankMode,
    buf: u8,
    cnt: usize,
}

#[derive(Serialize, Deserialize)]
enum PrgRomBankMode {
    Switch32K,
    Switch16KLow,
    Switch16KHigh,
}

#[derive(Serialize, Deserialize)]
enum ChrRomBankMode {
    Switch8K,
    Switch4K,
}

impl Mmc1 {
    pub fn new(ctx: &mut impl super::Context) -> Self {
        let prg_pages = ctx.memory_ctrl().prg_pages();
        ctx.map_prg(0, 0);
        ctx.map_prg(1, 1);
        ctx.map_prg(2, prg_pages - 2);
        ctx.map_prg(3, prg_pages - 1);

        Self {
            prg_rom_bank_mode: PrgRomBankMode::Switch16KLow,
            chr_rom_bank_mode: ChrRomBankMode::Switch8K,
            buf: 0,
            cnt: 0,
        }
    }
}

impl super::MapperTrait for Mmc1 {
    fn write_prg(&mut self, ctx: &mut impl super::Context, addr: u16, data: u8) {
        if addr & 0x8000 == 0 {
            ctx.write_prg(addr, data);
            return;
        }

        log::trace!("MMC1: {addr:04X} <- {data:02X}");

        if data & 0x80 != 0 {
            log::trace!("MMC1: Reset");
            self.buf = 0;
            self.cnt = 0;
            return;
        }

        self.buf |= (data & 1) << self.cnt;
        self.cnt += 1;

        if self.cnt < 5 {
            return;
        }

        let cmd = self.buf;
        self.buf = 0;
        self.cnt = 0;

        let reg_num = (addr >> 13) & 3;

        log::trace!("MMC1: reg[{reg_num}] <- ${cmd:02X} (b{cmd:05b})");

        match reg_num {
            0 => {
                ctx.memory_ctrl_mut().set_mirroring(match cmd & 0x3 {
                    0 => Mirroring::OneScreenLow,
                    1 => Mirroring::OneScreenHigh,
                    2 => Mirroring::Vertical,
                    3 => Mirroring::Horizontal,
                    _ => unreachable!(),
                });

                self.prg_rom_bank_mode = match (cmd >> 2) & 3 {
                    0 | 1 => PrgRomBankMode::Switch32K,
                    2 => PrgRomBankMode::Switch16KHigh,
                    3 => PrgRomBankMode::Switch16KLow,
                    _ => unreachable!(),
                };

                self.chr_rom_bank_mode = match (cmd >> 4) & 1 {
                    0 => ChrRomBankMode::Switch8K,
                    1 => ChrRomBankMode::Switch4K,
                    _ => unreachable!(),
                };
            }
            1 => match self.chr_rom_bank_mode {
                ChrRomBankMode::Switch8K => {
                    let page = (cmd >> 1) as u32;
                    for i in 0..8 {
                        ctx.map_chr(i, page * 8 + i);
                    }
                }
                ChrRomBankMode::Switch4K => {
                    let page = cmd as u32;
                    for i in 0..4 {
                        ctx.map_chr(i, page * 4 + i);
                    }
                }
            },
            2 => match self.chr_rom_bank_mode {
                ChrRomBankMode::Switch8K => {
                    log::info!("MMC1: High CHR page set on 8K CHR mode");
                }
                ChrRomBankMode::Switch4K => {
                    let page = cmd as u32;
                    for i in 0..4 {
                        ctx.map_chr(i + 4, page * 4 + i);
                    }
                }
            },
            3 => match self.prg_rom_bank_mode {
                PrgRomBankMode::Switch32K => {
                    let page = (cmd as u32 & 0x0f) >> 1;
                    for i in 0..4 {
                        ctx.map_prg(i, page * 4 + i);
                    }
                }
                PrgRomBankMode::Switch16KLow => {
                    let page = cmd as u32 & 0x0f;
                    let prg_pages = ctx.memory_ctrl().prg_pages();
                    for i in 0..2 {
                        ctx.map_prg(i, page * 2 + i);
                    }
                    ctx.map_prg(2, prg_pages - 2);
                    ctx.map_prg(3, prg_pages - 1);
                }
                PrgRomBankMode::Switch16KHigh => {
                    let page = cmd as u32 & 0x0f;
                    ctx.map_prg(0, 0);
                    ctx.map_prg(1, 1);
                    for i in 0..2 {
                        ctx.map_prg(i + 2, page * 2 + i);
                    }
                }
            },
            _ => unreachable!(),
        }
    }
}
