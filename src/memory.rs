use crate::{apu::Apu, mapper::Mapper, ppu::Ppu, rom::Rom, util::Ref};

pub struct MemoryMap {
    ram: Vec<u8>,
    ppu: Ref<Ppu>,
    apu: Ref<Apu>,
    mapper: Ref<dyn Mapper>,
}

impl MemoryMap {
    pub fn new(ppu: Ref<Ppu>, apu: Ref<Apu>, mapper: Ref<dyn Mapper>) -> Self {
        Self {
            ram: vec![0x00; 2 * 1024],
            ppu,
            apu,
            mapper,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize],
            0x2000..=0x3fff => self.ppu.borrow_mut().read_reg(addr & 7),
            0x4000..=0x4017 => self.apu.borrow_mut().read_reg(addr),
            0x4018..=0xffff => self.mapper.borrow_mut().read_prg(addr),
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1fff => self.ram[(addr & 0x7ff) as usize] = data,
            0x2000..=0x3fff => self.ppu.borrow_mut().write_reg(addr & 7, data),
            0x4000..=0x4013 | 0x4015..=0x4017 => self.apu.borrow_mut().write_reg(addr, data),
            0x4018..=0xffff => self.mapper.borrow_mut().write_prg(addr, data),

            0x4014 => {
                // OAM DMA
                log::warn!("OAM DMA = ${data:02X}");

                let hi = (data as u16) << 8;

                for lo in 0..0x100 {
                    let b = self.read(hi | lo);
                    self.write(0x2004, b);
                }

                // TODO: suspend cpu for 513 cycles
            }
        }
    }
}

pub struct MemoryController {
    rom: Ref<Rom>,

    prg_ram: Vec<u8>,
    chr_ram: Vec<u8>,

    nametable: [u8; 2 * 1024],
    palette: [u8; 0x20],

    rom_page: [usize; 4],
    chr_page: [usize; 8],
    nametable_page: [usize; 4],
}

impl MemoryController {
    pub fn new(rom: Ref<Rom>) -> Self {
        assert!(!(rom.borrow().chr_ram_size > 0 && !rom.borrow().chr_rom.is_empty()));

        let prg_ram = vec![0x00; rom.borrow().prg_ram_size];
        let chr_ram = vec![0x00; rom.borrow().chr_ram_size];

        let nametable = [0x00; 2 * 1024];

        #[rustfmt::skip]
        let palette = [
            0x09, 0x01, 0x00, 0x01, 0x00, 0x02, 0x02, 0x0D,
            0x08, 0x10, 0x08, 0x24, 0x00, 0x00, 0x04, 0x2C,
            0x09, 0x01, 0x34, 0x03, 0x00, 0x04, 0x00, 0x14,
            0x08, 0x3A, 0x00, 0x02, 0x00, 0x20, 0x2C, 0x08,
        ];

        let mut ret = Self {
            rom,
            prg_ram,
            chr_ram,
            nametable,
            palette,
            rom_page: [0; 4],
            chr_page: [0; 8],
            nametable_page: [0; 4],
        };

        for i in 0..4 {
            ret.map_prg(i, i);
        }

        for i in 0..8 {
            ret.map_chr(i, i);
        }

        ret
    }

    /// Maps a PRG ROM page to a given 8KB bank
    pub fn map_prg(&mut self, page: usize, bank: usize) {
        self.rom_page[page] = (bank * 0x2000) % self.rom.borrow().prg_rom.len();
    }

    pub fn prg_pages(&mut self) -> usize {
        self.rom.borrow().prg_rom.len() / 0x2000
    }

    /// Maps a CHR ROM page to a given 1KB bank
    pub fn map_chr(&mut self, page: usize, bank: usize) {
        if !self.rom.borrow().chr_rom.is_empty() {
            self.chr_page[page] = (bank * 0x0400) % self.rom.borrow().chr_rom.len();
        } else {
            self.chr_page[page] = (bank * 0x0400) % self.rom.borrow().chr_ram_size;
        }
    }

    pub fn chr_pages(&mut self) -> usize {
        self.rom.borrow().chr_rom.len() / 0x0400
    }

    pub fn set_mirroring(&mut self, page: usize, bank: usize) {
        todo!()
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7fff => {
                let addr = addr & 0x1fff;
                self.prg_ram[addr as usize]
            }
            0x8000..=0xffff => {
                let page = (addr & 0x7fff) / 0x2000;
                let ix = self.rom_page[page as usize] + (addr & 0x1fff) as usize;
                self.rom.borrow().prg_rom[ix]
            }
            _ => 0,
        }
    }

    pub fn write_prg(&mut self, addr: u16, data: u8) {
        match addr {
            0x6000..=0x7fff => {
                let addr = addr & 0x1fff;
                self.prg_ram[addr as usize] = data;
            }
            0x8000..=0xffff => {
                log::warn!("Write to PRG ROM: {addr:04x} = {data:02x}");
            }
            _ => (),
        }
    }

    pub fn read_chr(&self, addr: u16) -> u8 {
        log::trace!("Read CHR MEM: ${addr:04X}");

        match addr {
            0x0000..=0x1fff => {
                let page = (addr / 0x0400) as usize;
                let ix = self.chr_page[page] + (addr & 0x03ff) as usize;

                if !self.rom.borrow().chr_rom.is_empty() {
                    self.rom.borrow().chr_rom[ix]
                } else {
                    self.chr_ram[ix]
                }
            }
            0x2000..=0x3eff => {
                let page = (addr as usize & 0x0fff) / 0x400;
                let ofs = addr as usize & 0x03ff;
                let ix = self.nametable_page[page] + ofs;
                self.nametable[ix]
            }
            0x3f00..=0x3fff => {
                let addr = addr & if addr & 3 == 0 { 0x0f } else { 0x1f };
                self.palette[addr as usize]
            }
            _ => unreachable!(),
        }
    }

    pub fn write_chr(&mut self, addr: u16, data: u8) {
        log::trace!("Write CHR MEM: (${addr:04X}) = ${data:02X}");

        match addr {
            0x0000..=0x1fff => {
                let page = (addr / 0x0400) as usize;
                let ix = self.chr_page[page] + (addr & 0x03ff) as usize;

                if !self.rom.borrow().chr_rom.is_empty() {
                    log::warn!("Write to CHR ROM: (${addr:04X}) = ${data:02X}");
                } else {
                    self.chr_ram[ix] = data;
                }
            }
            0x2000..=0x3eff => {
                let page = (addr as usize & 0x0fff) / 0x400;
                let ofs = addr as usize & 0x03ff;
                let ix = self.nametable_page[page] + ofs;
                self.nametable[ix] = data;
            }
            0x3f00..=0x3fff => {
                let addr = addr & if addr & 3 == 0 { 0x0f } else { 0x1f };
                self.palette[addr as usize] = data;
            }
            _ => unreachable!(),
        }
    }
}
