use crate::util::{Input, Wire};

use bitvec::prelude::*;

const STEP_FRAME: [usize; 5] = [7457, 14913, 22371, 29829, 37281];

pub struct Apu {
    controller_latch: bool,
    expansion_latch: u8,
    pad_buf: [u8; 2],
    reg: Register,
    frame_counter_reset_delay: usize,
    frame_counter: usize,
    irq_line: Wire<bool>,
    input: Input,
}

#[derive(Default)]
struct Register {
    frame_counter_mode: bool,
    frame_counter_irq: bool,
}

impl Apu {
    pub fn new(irq_line: Wire<bool>) -> Self {
        Self {
            controller_latch: false,
            expansion_latch: 0,
            pad_buf: [0; 2],
            reg: Register::default(),
            frame_counter_reset_delay: 0,
            frame_counter: 0,
            input: Input::default(),
            irq_line,
        }
    }

    pub fn tick(&mut self) {
        self.frame_counter += 1;

        if self.frame_counter == STEP_FRAME[0] {
            // todo
        }
        if self.frame_counter == STEP_FRAME[1] {
            // todo
        }
        if self.frame_counter == STEP_FRAME[2] {
            // todo
        }
        if self.frame_counter == STEP_FRAME[3] {
            // todo

            if !self.reg.frame_counter_mode {
                if !self.reg.frame_counter_irq {
                    log::info!("APU frame counter IRQ set");
                    self.irq_line.set(true);
                }

                self.frame_counter = 0;
            }
        }
        if self.frame_counter == STEP_FRAME[4] {
            // todo
            self.frame_counter = 0;
        }

        if self.frame_counter_reset_delay > 0 {
            self.frame_counter_reset_delay -= 0;
            if self.frame_counter_reset_delay == 0 {
                self.frame_counter = 0;
            }
        }
    }

    pub fn set_input(&mut self, input: &Input) {
        self.input = input.clone();
    }

    pub fn read_reg(&mut self, addr: u16) -> u8 {
        let ret = match addr {
            0x4015 => {
                // Status
                let mut ret = 0;
                let r = ret.view_bits_mut::<Lsb0>();
                r.set(6, self.irq_line.get());

                self.irq_line.set(false);
                ret
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

            _ => {
                log::warn!("Read APU ${addr:04X}");
                0xA0
            }
        };
        log::info!("Read APU ${addr:04X} = {ret:02X}");
        ret
    }

    pub fn write_reg(&mut self, addr: u16, data: u8) {
        log::info!("Write APU ${addr:04X} = ${data:02X}");

        match addr {
            0x4016 => {
                let v = data.view_bits::<Lsb0>();
                self.controller_latch = v[0];
                self.expansion_latch = v[1..3].load_le();

                if self.controller_latch {
                    for (i, pad) in self.input.pad.iter().take(2).enumerate() {
                        let r = self.pad_buf[i].view_bits_mut::<Lsb0>();
                        r.set(0, pad.a);
                        r.set(1, pad.b);
                        r.set(2, pad.select);
                        r.set(3, pad.start);
                        r.set(4, pad.up);
                        r.set(5, pad.down);
                        r.set(6, pad.left);
                        r.set(7, pad.right);
                    }
                }
            }
            0x4017 => {
                let v = data.view_bits::<Lsb0>();
                self.reg.frame_counter_mode = v[7];
                self.reg.frame_counter_irq = v[6];

                if self.reg.frame_counter_irq {
                    self.irq_line.set(false);
                }

                self.frame_counter_reset_delay = 3;
            }
            _ => {
                log::warn!("Write APU ${addr:04X} = ${data:02X}");
            }
        }
    }
}
