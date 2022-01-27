use crate::{
    consts::{LINES_PER_FRAME, PPU_CLOCK_PER_CPU_CLOCK, PPU_CLOCK_PER_LINE},
    mapper::Mapper,
    util::{Input, Ref, Wire},
};

use bitvec::prelude::*;
use std::collections::VecDeque;

const AUDIO_FREQUENCY: u64 = 48000;
const SAMPLE_PER_FRAME: u64 = AUDIO_FREQUENCY / 60;
const STEP_FRAME: [usize; 5] = [7457, 14913, 22371, 29829, 37281];

#[rustfmt::skip]
const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14,
    12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
];

pub struct Apu {
    controller_latch: bool,
    expansion_latch: u8,
    pad_buf: [u8; 2],
    reg: Register,
    frame_counter_reset_delay: usize,
    frame_counter: usize,
    input: Input,
    counter: u64,
    sampler_counter: u64,
    mapper: Ref<dyn Mapper>,
    frame_irq_wire: Wire<bool>,
    dmc_irq_wire: Wire<bool>,
    pub audio_buf: VecDeque<i16>,
}

#[derive(Default)]
struct Register {
    pulse: [Pulse; 2],
    triangle: Triangle,
    noise: Noise,
    dmc: Dmc,

    frame_counter_mode: bool,
    frame_counter_irq: bool,
}

impl Register {
    fn new() -> Self {
        Register {
            noise: Noise::new(),
            dmc: Dmc::new(),
            ..Default::default()
        }
    }
}

#[derive(Default, Debug)]
struct Pulse {
    enable: bool,
    duty: u8,
    length_counter_halt: bool,
    constant_volume: bool,
    volume: u8,
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_reload: bool,
    timer: u16,
    length_counter_load: u8,

    sequencer_counter: u16,
    length_counter: u8,
    envelope_start: bool,
    envelope_counter: u8,
    decay_level: u8,
    sweep_counter: u8,
    phase: u8,
}

#[derive(Default)]
struct Triangle {
    enable: bool,
    length_counter_halt: bool,
    linear_counter_load: u8,
    timer: u16,
    length_counter_load: u8,

    length_counter: u8,
    phase: u8,
    linear_counter: u8,
    linear_counter_reload: bool,
    sequencer_counter: u16,
}

#[derive(Default, Debug)]
struct Noise {
    enable: bool,
    length_counter_halt: bool,
    constant_volume: bool,
    volume: u8,
    noise_mode: bool,
    noise_period: u8,
    length_counter_load: u8,

    length_counter: u8,
    envelope_start: bool,
    envelope_counter: u8,
    decay_level: u8,
    shift_register: u16,
    sequencer_counter: u16,
}

impl Noise {
    fn new() -> Noise {
        Noise {
            shift_register: 1,
            ..Default::default()
        }
    }
}

#[derive(Default)]
struct Dmc {
    enable: bool,
    irq_enabled: bool,
    loop_enabled: bool,
    rate_index: u8,
    sample_addr: u16,
    sample_length: u16,

    shifter_counter: u16,
    cur_addr: u16,
    length_counter: u16,
    shiftreg: u8,
    shiftreg_remain: u8,
    buffer: Option<u8>,
    silence: bool,
    output_level: u8,
}

impl Dmc {
    fn new() -> Self {
        Dmc {
            shiftreg_remain: 8,
            ..Default::default()
        }
    }
}

impl Apu {
    pub fn new(
        mapper: Ref<dyn Mapper>,
        frame_irq_wire: Wire<bool>,
        dmc_irq_wire: Wire<bool>,
    ) -> Self {
        Self {
            controller_latch: false,
            expansion_latch: 0,
            pad_buf: [0; 2],
            reg: Register::new(),
            frame_counter_reset_delay: 0,
            frame_counter: 0,
            counter: 0,
            sampler_counter: 0,
            input: Input::default(),
            mapper,
            frame_irq_wire,
            dmc_irq_wire,
            audio_buf: VecDeque::new(),
        }
    }

    pub fn tick(&mut self) {
        self.frame_counter += 1;

        let mut quarter_frame = false;
        let mut half_frame = false;

        if self.frame_counter == STEP_FRAME[0] {
            quarter_frame = true;
        }
        if self.frame_counter == STEP_FRAME[1] {
            quarter_frame = true;
            half_frame = true;
        }
        if self.frame_counter == STEP_FRAME[2] {
            quarter_frame = true;
        }
        if !self.reg.frame_counter_mode && self.frame_counter == STEP_FRAME[3] {
            quarter_frame = true;
            half_frame = true;

            if !self.reg.frame_counter_irq {
                // log::info!("APU frame counter IRQ set");
                self.frame_irq_wire.set(true);
            }

            self.frame_counter = 0;
        }
        if self.frame_counter == STEP_FRAME[4] {
            quarter_frame = true;
            half_frame = true;

            self.frame_counter = 0;
        }

        if self.frame_counter_reset_delay > 0 {
            self.frame_counter_reset_delay -= 1;
            if self.frame_counter_reset_delay == 0 {
                self.frame_counter = 0;
                if self.reg.frame_counter_mode {
                    quarter_frame = true;
                    half_frame = true;
                }
            }
        }

        // FIXME: delay clock frame
        if quarter_frame {
            self.clock_quarter_frame();
        }
        if half_frame {
            self.clock_half_frame();
        }

        self.counter += 1;

        if self.counter % 2 == 1 {
            for ch in 0..2 {
                let r = &mut self.reg.pulse[ch];
                if r.sequencer_counter == 0 {
                    r.sequencer_counter = r.timer;
                    r.phase = (r.phase + 1) % 8;
                } else {
                    r.sequencer_counter -= 1;
                }
            }
        }

        if self.reg.triangle.linear_counter != 0 && self.reg.triangle.length_counter != 0 {
            let r = &mut self.reg.triangle;
            if r.sequencer_counter == 0 {
                r.sequencer_counter = r.timer;
                r.phase = (r.phase + 1) % 32;
            } else {
                r.sequencer_counter -= 1;
            }
        }

        if self.counter % 2 == 1 {
            const NOISE_PERIOD: [u16; 16] = [
                4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
            ];

            let r = &mut self.reg.noise;
            if r.sequencer_counter == 0 {
                r.sequencer_counter = NOISE_PERIOD[r.noise_period as usize];
                let fb = if !r.noise_mode {
                    (r.shift_register & 1) ^ ((r.shift_register >> 1) & 1)
                } else {
                    (r.shift_register & 1) ^ ((r.shift_register >> 6) & 1)
                };
                r.shift_register = (r.shift_register >> 1) | (fb << 14);
            } else {
                r.sequencer_counter -= 1;
            }
        }

        {
            const RATE_TABLE: [u16; 16] = [
                428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
            ];

            let r = &mut self.reg.dmc;
            if r.shifter_counter == 0 {
                r.shifter_counter = RATE_TABLE[r.rate_index as usize];

                if !r.silence {
                    if r.shiftreg & 1 != 0 {
                        if r.output_level <= 0x7d {
                            r.output_level += 2;
                        }
                    } else {
                        if r.output_level >= 2 {
                            r.output_level -= 2;
                        }
                    }
                    r.shiftreg >>= 1;
                }

                r.shiftreg_remain -= 1;
                if r.shiftreg_remain == 0 {
                    r.shiftreg_remain = 8;

                    if let Some(buf) = r.buffer {
                        r.shiftreg = buf;
                        r.buffer = None;
                        r.silence = false;
                    } else {
                        r.silence = true;
                    }
                }
            } else {
                r.shifter_counter -= 1;
            }

            if r.buffer.is_none() && r.length_counter != 0 {
                r.buffer = Some(self.mapper.borrow_mut().read_prg(r.cur_addr));

                r.cur_addr = r.cur_addr.wrapping_add(1);
                if r.cur_addr == 0 {
                    r.cur_addr = 0x8000;
                }
                r.length_counter -= 1;
                if r.length_counter == 0 {
                    if r.loop_enabled {
                        r.cur_addr = r.sample_addr;
                        r.length_counter = r.sample_length;
                    } else if r.irq_enabled {
                        self.dmc_irq_wire.set(true);
                    }
                }
            }
        }

        // PPU_CLOCK_PER_LINE * LINES_PER_FRAME <-> 800 * 3

        self.sampler_counter += SAMPLE_PER_FRAME * PPU_CLOCK_PER_CPU_CLOCK;
        if self.sampler_counter >= PPU_CLOCK_PER_LINE * LINES_PER_FRAME as u64 {
            self.sampler_counter -= PPU_CLOCK_PER_LINE * LINES_PER_FRAME as u64;
            self.audio_buf.push_back(self.sample());
        }
    }

    pub fn clock_quarter_frame(&mut self) {
        for i in 0..2 {
            let r = &mut self.reg.pulse[i];

            if r.envelope_start {
                r.envelope_start = false;
                r.decay_level = 15;
                r.envelope_counter = r.volume;
            } else if r.volume > 0 {
                if r.envelope_counter == 0 {
                    r.envelope_counter = r.volume;
                    if r.decay_level != 0 {
                        r.decay_level -= 1;
                    } else if r.length_counter_halt {
                        r.decay_level = 15;
                    }
                } else {
                    r.envelope_counter -= 1;
                }
            }
        }

        let r = &mut self.reg.triangle;
        if r.linear_counter_reload {
            r.linear_counter = r.linear_counter_load;
        } else if r.linear_counter > 0 {
            r.linear_counter -= 1;
        }
        if !r.length_counter_halt {
            r.linear_counter_reload = false;
        }

        let r = &mut self.reg.noise;
        if r.envelope_start {
            r.envelope_start = false;
            r.decay_level = 15;
            r.envelope_counter = r.volume;
        } else if r.volume > 0 {
            if r.envelope_counter == 0 {
                r.envelope_counter = r.volume;
                if r.decay_level != 0 {
                    r.decay_level -= 1;
                } else if r.length_counter_halt {
                    r.decay_level = 15;
                }
            } else {
                r.envelope_counter -= 1;
            }
        }
    }

    pub fn clock_half_frame(&mut self) {
        for ch in 0..2 {
            let target_period = self.target_period(ch);

            let r = &mut self.reg.pulse[ch];
            if r.length_counter > 0 && !r.length_counter_halt {
                r.length_counter -= 1;
            }

            let enabled = r.sweep_enabled && r.sweep_shift != 0;
            let muting = target_period < 8 || target_period > 0x7ff;

            if r.sweep_counter == 0 && enabled && !muting {
                r.timer = target_period;
            }

            if r.sweep_counter == 0 || r.sweep_reload {
                r.sweep_counter = r.sweep_period;
                r.sweep_reload = false;
            } else {
                r.sweep_counter -= 1;
            }
        }
        if self.reg.triangle.length_counter > 0 && !self.reg.triangle.length_counter_halt {
            self.reg.triangle.length_counter -= 1;
        }
        if self.reg.noise.length_counter > 0 && !self.reg.noise.length_counter_halt {
            self.reg.noise.length_counter -= 1;
        }
    }

    pub fn sample(&self) -> i16 {
        const PULSE_WAVEFORM: [[u8; 8]; 4] = [
            [0, 1, 0, 0, 0, 0, 0, 0],
            [0, 1, 1, 0, 0, 0, 0, 0],
            [0, 1, 1, 1, 1, 0, 0, 0],
            [1, 0, 0, 1, 1, 1, 1, 1],
        ];

        let mut pulse = [0, 0];

        for ch in 0..2 {
            let r = &self.reg.pulse[ch];
            let volume = if r.constant_volume {
                r.volume
            } else {
                r.decay_level
            };
            let target_period = self.target_period(ch);
            let sweep_muting = r.sweep_enabled && (target_period < 8 || target_period > 0x7ff);
            if !(r.length_counter == 0 || sweep_muting || r.timer < 8) {
                pulse[ch] = volume * PULSE_WAVEFORM[r.duty as usize][r.phase as usize];
            }
        }

        let pulse_out = if pulse[0] == 0 && pulse[1] == 0 {
            0.0
        } else {
            95.88 / (8128.0 / (pulse[0] as f64 + pulse[1] as f64) + 100.0)
        };

        #[rustfmt::skip]
        const TRIANGLE_WAVEFORM: [u8; 32] = [
            15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ];

        // mute when timer value is too small because it produces ultrasonic
        let triangle = if self.reg.triangle.linear_counter == 0
            || self.reg.triangle.length_counter == 0
            || self.reg.triangle.timer <= 2
        {
            0
        } else {
            TRIANGLE_WAVEFORM[self.reg.triangle.phase as usize]
        };

        let noise = {
            let r = &self.reg.noise;
            let volume = if r.constant_volume {
                r.volume
            } else {
                r.decay_level
            };
            if !(r.length_counter == 0 || r.shift_register & 1 == 1) {
                volume
            } else {
                0
            }
        };

        let dmc = 0;

        let tnd_out = if triangle == 0 && noise == 0 && dmc == 0 {
            0.0
        } else {
            let t = triangle as f64 / 8227.0 + noise as f64 / 12241.0 + dmc as f64 / 22638.0;
            159.79 / (1.0 / t + 100.0)
        };

        // TODO: highpass filter & lowpass filter
        ((pulse_out + tnd_out) * 30000.0).round() as i16
    }

    fn target_period(&self, ch: usize) -> u16 {
        let r = &self.reg.pulse[ch];
        let delta = r.timer >> r.sweep_shift;
        if !r.sweep_negate {
            r.timer + delta
        } else if ch == 0 {
            r.timer - delta - 1
        } else {
            r.timer - delta
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
                r.set(7, self.dmc_irq_wire.get());
                r.set(6, self.frame_irq_wire.get());
                r.set(4, self.reg.dmc.length_counter > 0);
                r.set(3, self.reg.noise.length_counter > 0);
                r.set(2, self.reg.triangle.length_counter > 0);
                r.set(1, self.reg.pulse[1].length_counter > 0);
                r.set(0, self.reg.pulse[0].length_counter > 0);

                self.frame_irq_wire.set(false);
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
        log::trace!("Read APU ${addr:04X} = {ret:02X}");
        ret
    }

    pub fn write_reg(&mut self, addr: u16, data: u8) {
        log::trace!("Write APU ${addr:04X} = ${data:02X}");

        match addr {
            // Pulse
            0x4000 | 0x4004 => {
                let ch = (addr - 0x4000) / 4;
                let r = &mut self.reg.pulse[ch as usize];
                let v = data.view_bits::<Lsb0>();
                r.duty = v[6..8].load();
                r.length_counter_halt = v[5];
                r.constant_volume = v[4];
                r.volume = v[0..4].load();

                log::trace!(
                    "Pulse #{ch}: duty={}, inflen={}, constvol={}, vol={}",
                    r.duty,
                    r.length_counter_halt,
                    r.constant_volume,
                    r.volume
                );
            }
            0x4001 | 0x4005 => {
                let ch = (addr - 0x4000) / 4;
                let r = &mut self.reg.pulse[ch as usize];
                let v = data.view_bits::<Lsb0>();
                r.sweep_enabled = v[7];
                r.sweep_period = v[4..6].load();
                r.sweep_negate = v[3];
                r.sweep_shift = v[0..3].load();
                r.sweep_reload = true;

                log::trace!(
                    "Pulse #{ch}: swenable={}, swperiod={}, swneg={}, swshft={}, swreload={}",
                    r.sweep_enabled,
                    r.sweep_period,
                    r.sweep_negate,
                    r.sweep_shift,
                    r.sweep_reload
                );
            }
            0x4002 | 0x4006 => {
                let ch = (addr - 0x4000) / 4;
                let r = &mut self.reg.pulse[ch as usize];
                r.timer.view_bits_mut::<Lsb0>()[0..8].store(data);

                log::trace!("Pulse #{ch}: timer_low={}, timer={}", data, r.timer);
            }
            0x4003 | 0x4007 => {
                let ch = (addr - 0x4000) / 4;
                let r = &mut self.reg.pulse[ch as usize];
                let v = data.view_bits::<Lsb0>();
                r.timer.view_bits_mut::<Lsb0>()[8..].store(v[0..3].load::<u8>());
                r.length_counter_load = v[3..8].load();

                if r.enable {
                    r.length_counter = LENGTH_TABLE[r.length_counter_load as usize];
                    log::trace!("PULSE {ch}: length: {}", r.length_counter);
                }
                r.envelope_start = true;
                r.phase = 0;

                log::trace!(
                    "Pulse #{ch}: timer_high={}, timer={}, length={}, enabled={}",
                    v[0..3].load::<u8>(),
                    r.timer,
                    r.length_counter_load,
                    r.enable,
                );
            }

            // Triangle
            0x4008 => {
                let r = &mut self.reg.triangle;
                let v = data.view_bits::<Lsb0>();
                r.length_counter_halt = v[7];
                r.linear_counter_load = v[0..7].load();
            }
            0x4009 => {
                log::warn!("Write APU ${addr:04X} = ${data:02X}");
            }
            0x400A => {
                let r = &mut self.reg.triangle;
                r.timer.view_bits_mut::<Lsb0>()[0..8].store(data);
            }
            0x400B => {
                let r = &mut self.reg.triangle;
                let v = data.view_bits::<Lsb0>();
                r.timer.view_bits_mut::<Lsb0>()[8..].store(v[0..3].load::<u8>());
                r.length_counter_load = v[3..8].load();
                if r.enable {
                    r.length_counter = LENGTH_TABLE[r.length_counter_load as usize];
                }
                r.linear_counter_reload = true;
            }

            // Noise
            0x400C => {
                let r = &mut self.reg.noise;
                let v = data.view_bits::<Lsb0>();
                r.length_counter_halt = v[5];
                r.constant_volume = v[4];
                r.volume = v[0..4].load();
            }
            0x400D => {
                log::warn!("Write APU ${addr:04X} = ${data:02X}");
            }
            0x400E => {
                let r = &mut self.reg.noise;
                let v = data.view_bits::<Lsb0>();
                r.noise_mode = v[7];
                r.noise_period = v[0..4].load();
            }
            0x400F => {
                let r = &mut self.reg.noise;
                let v = data.view_bits::<Lsb0>();
                r.length_counter_load = v[3..8].load();
                if r.enable {
                    r.length_counter = LENGTH_TABLE[r.length_counter_load as usize];
                }
                r.envelope_start = true;
            }

            // DMC
            0x4010 => {
                let r = &mut self.reg.dmc;
                let v = data.view_bits::<Lsb0>();
                r.irq_enabled = v[7];
                r.loop_enabled = v[6];
                r.rate_index = v[0..4].load();
                if !r.irq_enabled {
                    self.dmc_irq_wire.set(false);
                }
            }
            0x4011 => {
                let r = &mut self.reg.dmc;
                let v = data.view_bits::<Lsb0>();
                r.output_level = v[0..7].load();
            }
            0x4012 => {
                let r = &mut self.reg.dmc;
                r.sample_addr = 0xC000 + data as u16 * 64;
            }
            0x4013 => {
                let r = &mut self.reg.dmc;
                r.sample_length = data as u16 * 16 + 1;
            }

            // Status
            0x4015 => {
                let v = data.view_bits::<Lsb0>();
                self.reg.pulse[0].enable = v[0];
                self.reg.pulse[1].enable = v[1];
                self.reg.triangle.enable = v[2];
                self.reg.noise.enable = v[3];
                self.reg.dmc.enable = v[4];

                for i in 0..2 {
                    if !self.reg.pulse[i].enable {
                        self.reg.pulse[i].length_counter = 0;
                    }
                }
                if !self.reg.triangle.enable {
                    self.reg.triangle.length_counter = 0;
                }
                if !self.reg.noise.enable {
                    self.reg.noise.length_counter = 0;
                }

                if !self.reg.dmc.enable {
                    self.reg.dmc.length_counter = 0;
                } else {
                    if self.reg.dmc.length_counter == 0 {
                        self.reg.dmc.cur_addr = self.reg.dmc.sample_addr;
                        self.reg.dmc.length_counter = self.reg.dmc.sample_length;
                    }
                }

                self.dmc_irq_wire.set(false);
            }

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
                    self.frame_irq_wire.set(false);
                }

                self.frame_counter_reset_delay = 3;
            }

            _ => {
                log::warn!("Write APU ${addr:04X} = ${data:02X}");
            }
        }
    }
}
