use crate::{
    memory::MemoryController,
    rom::{Mirroring, Rom},
    util::Ref,
};

pub struct Mmc1 {
    prg_rom_bank_mode: PrgRomBankMode,
    chr_rom_bank_mode: ChrRomBankMode,
    buf: u8,
    cnt: usize,
    ctrl: MemoryController,
}

enum PrgRomBankMode {
    Switch32K,
    Switch16KLow,
    Switch16KHigh,
}

enum ChrRomBankMode {
    Switch8K,
    Switch4K,
}

impl Mmc1 {
    pub fn new(rom: Ref<Rom>) -> Self {
        let mut ctrl = MemoryController::new(rom);
        let prg_pages = ctrl.prg_pages();
        ctrl.map_prg(0, 0);
        ctrl.map_prg(1, 1);
        ctrl.map_prg(2, prg_pages - 2);
        ctrl.map_prg(3, prg_pages - 1);

        Self {
            prg_rom_bank_mode: PrgRomBankMode::Switch16KLow,
            chr_rom_bank_mode: ChrRomBankMode::Switch8K,
            buf: 0,
            cnt: 0,
            ctrl,
        }
    }
}

impl super::Mapper for Mmc1 {
    fn read_prg(&mut self, addr: u16) -> u8 {
        self.ctrl.read_prg(addr)
    }

    fn read_chr(&mut self, addr: u16) -> u8 {
        self.ctrl.read_chr(addr)
    }

    fn write_prg(&mut self, addr: u16, data: u8) {
        if addr & 0x8000 == 0 {
            return self.ctrl.write_prg(addr, data);
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
                self.ctrl.set_mirroring(match cmd & 0x3 {
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
                    let page = (cmd >> 1) as usize;
                    for i in 0..8 {
                        self.ctrl.map_chr(i, page as usize * 8 + i);
                    }
                }
                ChrRomBankMode::Switch4K => {
                    let page = cmd as usize;
                    for i in 0..4 {
                        self.ctrl.map_chr(i, page as usize * 4 + i);
                    }
                }
            },
            2 => match self.chr_rom_bank_mode {
                ChrRomBankMode::Switch8K => {
                    log::warn!("MMC1: High CHR page set on 8K CHR mode");
                }
                ChrRomBankMode::Switch4K => {
                    let page = cmd as usize;
                    for i in 0..4 {
                        self.ctrl.map_chr(i + 4, page as usize * 4 + i);
                    }
                }
            },
            3 => match self.prg_rom_bank_mode {
                PrgRomBankMode::Switch32K => {
                    let page = (cmd as usize & 0x0f) >> 1;
                    for i in 0..4 {
                        self.ctrl.map_prg(i, page * 4 + i);
                    }
                }
                PrgRomBankMode::Switch16KLow => {
                    let page = cmd as usize & 0x0f;
                    let prg_pages = self.ctrl.prg_pages();
                    for i in 0..2 {
                        self.ctrl.map_prg(i, page * 2 + i);
                    }
                    self.ctrl.map_prg(2, prg_pages - 2);
                    self.ctrl.map_prg(3, prg_pages - 1);
                }
                PrgRomBankMode::Switch16KHigh => {
                    let page = cmd as usize & 0x0f;
                    self.ctrl.map_prg(0, 0);
                    self.ctrl.map_prg(1, 1);
                    for i in 0..2 {
                        self.ctrl.map_prg(i + 2, page * 2 + i);
                    }
                }
            },
            _ => unreachable!(),
        }
    }

    fn write_chr(&mut self, addr: u16, data: u8) {
        self.ctrl.write_chr(addr, data);
    }

    fn get_prg_page(&self, page: usize) -> usize {
        self.ctrl.get_prg_page(page)
    }
}
