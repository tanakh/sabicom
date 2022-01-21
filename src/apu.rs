use crate::util::Input;

pub struct Apu {
    controller_latch: bool,
    expansion_latch: u8,
    pad_buf: [u8; 2],
    input: Input,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            controller_latch: false,
            expansion_latch: 0,
            pad_buf: [0; 2],
            input: Input::default(),
        }
    }

    pub fn set_input(&mut self, input: &Input) {
        self.input = input.clone();
    }

    pub fn read_reg(&mut self, addr: u16) -> u8 {
        let ret = match addr {
            0x4015 => {
                // todo
                0
            }

            0x4016 | 0x4017 => {
                let ix = (addr - 0x4016) as usize;

                if self.controller_latch {
                    0x00
                } else {
                    let ret = self.pad_buf[ix] & 1 != 0;
                    self.pad_buf[ix] = self.pad_buf[ix] >> 1 | 0x80;
                    ret as u8
                }
            }

            _ => 0xA0,
        };
        log::info!("Read APU ${addr:04X} = {ret:02X}");
        ret
    }

    pub fn write_reg(&mut self, addr: u16, data: u8) {
        log::info!("Write APU ${addr:04X} = ${data:02X}");

        match addr {
            0x4016 => {
                self.controller_latch = data & 0x01 != 0;
                self.expansion_latch = (data >> 1) & 3;

                if self.controller_latch {
                    for i in 0..2 {
                        self.pad_buf[i] = 0;
                        self.pad_buf[i] |= if self.input.pad[i].a { 0x01 } else { 0 };
                        self.pad_buf[i] |= if self.input.pad[i].b { 0x02 } else { 0 };
                        self.pad_buf[i] |= if self.input.pad[i].select { 0x04 } else { 0 };
                        self.pad_buf[i] |= if self.input.pad[i].start { 0x08 } else { 0 };
                        self.pad_buf[i] |= if self.input.pad[i].up { 0x10 } else { 0 };
                        self.pad_buf[i] |= if self.input.pad[i].down { 0x20 } else { 0 };
                        self.pad_buf[i] |= if self.input.pad[i].left { 0x40 } else { 0 };
                        self.pad_buf[i] |= if self.input.pad[i].right { 0x80 } else { 0 };
                    }
                }
            }
            0x4017 => {
                log::warn!("Invalid write to APU ${addr:04X} = ${data:02X}");
            }
            _ => {}
        }
    }
}
