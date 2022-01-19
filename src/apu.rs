pub struct Apu {}

impl Apu {
    pub fn new() -> Self {
        Self {}
    }

    pub fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0x4015 => {
                log::info!("Read APU ${addr:04X}");
                0
            }
            _ => 0xA0,
        }
    }

    pub fn write_reg(&mut self, addr: u16, data: u8) {
        log::info!("Write APU ${addr:04X} = ${data:02X}");
    }
}
