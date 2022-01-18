pub struct Apu {}

impl Apu {
    pub fn new() -> Self {
        Self {}
    }

    pub fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0x4015 => {
                log::warn!("Read APU ${addr:04X}");
                0
            }
            _ => 0xA0,
        }
    }

    pub fn write_reg(&mut self, addr: u16, val: u8) {
        log::warn!("Write APU ${addr:04X} = ${val:02X}");
    }
}
