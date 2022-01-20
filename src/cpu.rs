use crate::{
    memory::MemoryMap,
    util::{Ref, Wire},
};

const NMI_VECTOR: u16 = 0xFFFA;
const RST_VECTOR: u16 = 0xFFFC;
const IRQ_VECTOR: u16 = 0xFFFE;

pub struct Cpu {
    nmi_prev: bool,

    world: u64,
    counter: u64,

    reg: Register,

    mem: Ref<MemoryMap>,
    wires: Wires,
}

pub struct Wires {
    pub nmi: Wire<bool>,
    pub irq: Wire<bool>,
    pub rst: Wire<bool>,
}

#[derive(Debug)]
pub enum Interrupt {
    Rst,
    Irq,
    Nmi,
}

struct Register {
    a: u8,
    x: u8,
    y: u8,
    s: u8,
    pc: u16,
    flag: Flag,
}

impl Register {
    fn new() -> Self {
        Register {
            a: 0,
            x: 0,
            y: 0,
            s: 0,
            pc: 0,
            flag: Flag::new(),
        }
    }
}

struct Flag {
    c: bool,
    z: bool,
    i: bool,
    d: bool,
    v: bool,
    n: bool,
}

impl Flag {
    fn new() -> Self {
        Self {
            c: false,
            z: false,
            i: false,
            d: false,
            v: false,
            n: false,
        }
    }

    fn set_u8(&mut self, v: u8) {
        self.n = (v & 0x80) != 0;
        self.v = (v & 0x40) != 0;
        self.d = (v & 0x08) != 0;
        self.i = (v & 0x04) != 0;
        self.z = (v & 0x02) != 0;
        self.c = (v & 0x01) != 0;
    }

    fn get_u8(&self, b: u8) -> u8 {
        let mut v = 0x20;
        v |= if self.n { 0x80 } else { 0 };
        v |= if self.v { 0x40 } else { 0 };
        v |= b << 4;
        v |= if self.d { 0x08 } else { 0 };
        v |= if self.i { 0x04 } else { 0 };
        v |= if self.z { 0x02 } else { 0 };
        v |= if self.c { 0x01 } else { 0 };
        v
    }

    fn set_nz(&mut self, v: u8) {
        self.z = v == 0;
        self.n = v & 0x80 != 0;
    }
}

impl Cpu {
    pub fn new(mem: Ref<MemoryMap>, wires: Wires) -> Self {
        let mut ret = Self {
            mem,
            counter: 0,
            world: 0,
            reg: Register::new(),
            wires,
            nmi_prev: false,
        };
        ret.exec_interrupt(Interrupt::Rst, false);

        if log::log_enabled!(target: "disasm-nestest", log::Level::Trace) {
            ret.reg.pc = 0xC000;
        }

        ret
    }

    fn exec_interrupt(&mut self, interrupt: Interrupt, brk: bool) {
        log::info!("Interrupt: {:?}", interrupt);

        let vect = match interrupt {
            Interrupt::Rst => RST_VECTOR,
            Interrupt::Irq => IRQ_VECTOR,
            Interrupt::Nmi => NMI_VECTOR,
        };

        self.push_u16(self.reg.pc);
        self.push_u8(self.reg.flag.get_u8(if brk { 3 } else { 2 }));
        self.reg.pc = self.read_u8(vect) as u16 | (self.read_u8(vect + 1) as u16) << 8;
        self.reg.flag.i = true;
    }

    fn read_u8(&mut self, addr: u16) -> u8 {
        self.counter += 1;
        let ret = self.mem.borrow().read(addr);
        log::info!(target: "prgmem", "[${addr:04X}] -> ${ret:02X}");
        ret
    }

    fn write_u8(&mut self, addr: u16, data: u8) {
        self.counter += 1;
        self.mem.borrow_mut().write(addr, data);
        log::info!(target: "prgmem", "[${addr:04X}] <- ${data:02X}");
    }

    fn fetch_u8(&mut self) -> u8 {
        let ret = self.read_u8(self.reg.pc);
        self.reg.pc = self.reg.pc.wrapping_add(1);
        ret
    }

    fn fetch_u16(&mut self) -> u16 {
        let lo = self.fetch_u8();
        let hi = self.fetch_u8();
        lo as u16 | (hi as u16) << 8
    }

    fn push_u8(&mut self, data: u8) {
        self.write_u8(0x100 + self.reg.s as u16, data);
        self.reg.s = self.reg.s.wrapping_sub(1);
    }

    fn push_u16(&mut self, data: u16) {
        self.push_u8((data >> 8) as u8);
        self.push_u8(data as u8);
    }

    fn pop_u8(&mut self) -> u8 {
        self.reg.s = self.reg.s.wrapping_add(1);
        self.read_u8(0x100 + self.reg.s as u16)
    }

    fn pop_u16(&mut self) -> u16 {
        let lo = self.pop_u8() as u16;
        let hi = self.pop_u8() as u16;
        lo | (hi << 8)
    }
}

enum AddrMode {
    IMP, // Implicit
    ACC, // Accumulator
    IMM, // Immediate: #v
    ZPG, // Zero Page: d
    ABS, // Absolute: a
    REL, // Relative: label
    IND, // Indirect: (d)
    ZPX, // Zero Page indexed: d,X
    ZPY, // Zero Page indexed: d,Y
    ABX, // Absolute indexed: a,X
    ABY, // Absolute indexed: a,Y
    INX, // Indirect indexed: (d,X)
    INY, // Indirect indexed: (d),Y
    UNK,
}

impl AddrMode {
    fn len(&self) -> usize {
        use AddrMode::*;
        match self {
            IMP | ACC => 1,
            IMM | ZPG | REL | ZPX | ZPY | INX | INY => 2,
            ABS | IND | ABX | ABY => 3,
            UNK => 1,
        }
    }
}

macro_rules! instructions {
    ($cont:ident) => {
        $cont! {
            0x00: BRK IMM, 0x01: ORA INX, 0x02: UNK UNK, 0x03: UNK UNK,
            0x04: UNK UNK, 0x05: ORA ZPG, 0x06: ASL ZPG, 0x07: UNK UNK,
            0x08: PHP IMP, 0x09: ORA IMM, 0x0A: ASL ACC, 0x0B: UNK UNK,
            0x0C: UNK UNK, 0x0D: ORA ABS, 0x0E: ASL ABS, 0x0F: UNK UNK,
            0x10: BPL REL, 0x11: ORA INY, 0x12: UNK UNK, 0x13: UNK UNK,
            0x14: UNK UNK, 0x15: ORA ZPX, 0x16: ASL ZPX, 0x17: UNK UNK,
            0x18: CLC IMP, 0x19: ORA ABY, 0x1A: UNK UNK, 0x1B: UNK UNK,
            0x1C: UNK UNK, 0x1D: ORA ABX, 0x1E: ASL ABX, 0x1F: UNK UNK,
            0x20: JSR ABS, 0x21: AND INX, 0x22: UNK UNK, 0x23: UNK UNK,
            0x24: BIT ZPG, 0x25: AND ZPG, 0x26: ROL ZPG, 0x27: UNK UNK,
            0x28: PLP IMP, 0x29: AND IMM, 0x2A: ROL ACC, 0x2B: UNK UNK,
            0x2C: BIT ABS, 0x2D: AND ABS, 0x2E: ROL ABS, 0x2F: UNK UNK,
            0x30: BMI REL, 0x31: AND INY, 0x32: UNK UNK, 0x33: UNK UNK,
            0x34: UNK UNK, 0x35: AND ZPX, 0x36: ROL ZPX, 0x37: UNK UNK,
            0x38: SEC IMP, 0x39: AND ABY, 0x3A: UNK UNK, 0x3B: UNK UNK,
            0x3C: UNK UNK, 0x3D: AND ABX, 0x3E: ROL ABX, 0x3F: UNK UNK,
            0x40: RTI IMP, 0x41: EOR INX, 0x42: UNK UNK, 0x43: UNK UNK,
            0x44: UNK UNK, 0x45: EOR ZPG, 0x46: LSR ZPG, 0x47: UNK UNK,
            0x48: PHA IMP, 0x49: EOR IMM, 0x4A: LSR ACC, 0x4B: UNK UNK,
            0x4C: JMP ABS, 0x4D: EOR ABS, 0x4E: LSR ABS, 0x4F: UNK UNK,
            0x50: BVC REL, 0x51: EOR INY, 0x52: UNK UNK, 0x53: UNK UNK,
            0x54: UNK UNK, 0x55: EOR ZPX, 0x56: LSR ZPX, 0x57: UNK UNK,
            0x58: CLI IMP, 0x59: EOR ABY, 0x5A: UNK UNK, 0x5B: UNK UNK,
            0x5C: UNK UNK, 0x5D: EOR ABX, 0x5E: LSR ABX, 0x5F: UNK UNK,
            0x60: RTS IMP, 0x61: ADC INX, 0x62: UNK UNK, 0x63: UNK UNK,
            0x64: UNK UNK, 0x65: ADC ZPG, 0x66: ROR ZPG, 0x67: UNK UNK,
            0x68: PLA IMP, 0x69: ADC IMM, 0x6A: ROR ACC, 0x6B: UNK UNK,
            0x6C: JMP IND, 0x6D: ADC ABS, 0x6E: ROR ABS, 0x6F: UNK UNK,
            0x70: BVS REL, 0x71: ADC INY, 0x72: UNK UNK, 0x73: UNK UNK,
            0x74: UNK UNK, 0x75: ADC ZPX, 0x76: ROR ZPX, 0x77: UNK UNK,
            0x78: SEI IMP, 0x79: ADC ABY, 0x7A: UNK UNK, 0x7B: UNK UNK,
            0x7C: UNK UNK, 0x7D: ADC ABX, 0x7E: ROR ABX, 0x7F: UNK UNK,
            0x80: UNK UNK, 0x81: STA INX, 0x82: UNK UNK, 0x83: UNK UNK,
            0x84: STY ZPG, 0x85: STA ZPG, 0x86: STX ZPG, 0x87: UNK UNK,
            0x88: DEY IMP, 0x89: UNK UNK, 0x8A: TXA IMP, 0x8B: UNK UNK,
            0x8C: STY ABS, 0x8D: STA ABS, 0x8E: STX ABS, 0x8F: UNK UNK,
            0x90: BCC REL, 0x91: STA INY, 0x92: UNK UNK, 0x93: UNK UNK,
            0x94: STY ZPX, 0x95: STA ZPX, 0x96: STX ZPY, 0x97: UNK UNK,
            0x98: TYA IMP, 0x99: STA ABY, 0x9A: TXS IMP, 0x9B: UNK UNK,
            0x9C: UNK UNK, 0x9D: STA ABX, 0x9E: UNK UNK, 0x9F: UNK UNK,
            0xA0: LDY IMM, 0xA1: LDA INX, 0xA2: LDX IMM, 0xA3: UNK UNK,
            0xA4: LDY ZPG, 0xA5: LDA ZPG, 0xA6: LDX ZPG, 0xA7: UNK UNK,
            0xA8: TAY IMP, 0xA9: LDA IMM, 0xAA: TAX IMP, 0xAB: UNK UNK,
            0xAC: LDY ABS, 0xAD: LDA ABS, 0xAE: LDX ABS, 0xAF: UNK UNK,
            0xB0: BCS REL, 0xB1: LDA INY, 0xB2: UNK UNK, 0xB3: UNK UNK,
            0xB4: LDY ZPX, 0xB5: LDA ZPX, 0xB6: LDX ZPY, 0xB7: UNK UNK,
            0xB8: CLV IMP, 0xB9: LDA ABY, 0xBA: TSX IMP, 0xBB: UNK UNK,
            0xBC: LDY ABX, 0xBD: LDA ABX, 0xBE: LDX ABY, 0xBF: UNK UNK,
            0xC0: CPY IMM, 0xC1: CMP INX, 0xC2: UNK UNK, 0xC3: UNK UNK,
            0xC4: CPY ZPG, 0xC5: CMP ZPG, 0xC6: DEC ZPG, 0xC7: UNK UNK,
            0xC8: INY IMP, 0xC9: CMP IMM, 0xCA: DEX IMP, 0xCB: UNK UNK,
            0xCC: CPY ABS, 0xCD: CMP ABS, 0xCE: DEC ABS, 0xCF: UNK UNK,
            0xD0: BNE REL, 0xD1: CMP INY, 0xD2: UNK UNK, 0xD3: UNK UNK,
            0xD4: UNK UNK, 0xD5: CMP ZPX, 0xD6: DEC ZPX, 0xD7: UNK UNK,
            0xD8: CLD IMP, 0xD9: CMP ABY, 0xDA: UNK UNK, 0xDB: UNK UNK,
            0xDC: UNK UNK, 0xDD: CMP ABX, 0xDE: DEC ABX, 0xDF: UNK UNK,
            0xE0: CPX IMM, 0xE1: SBC INX, 0xE2: UNK UNK, 0xE3: UNK UNK,
            0xE4: CPX ZPG, 0xE5: SBC ZPG, 0xE6: INC ZPG, 0xE7: UNK UNK,
            0xE8: INX IMP, 0xE9: SBC IMM, 0xEA: NOP IMP, 0xEB: UNK UNK,
            0xEC: CPX ABS, 0xED: SBC ABS, 0xEE: INC ABS, 0xEF: UNK UNK,
            0xF0: BEQ REL, 0xF1: SBC INY, 0xF2: UNK UNK, 0xF3: UNK UNK,
            0xF4: UNK UNK, 0xF5: SBC ZPX, 0xF6: INC ZPX, 0xF7: UNK UNK,
            0xF8: SED IMP, 0xF9: SBC ABY, 0xFA: UNK UNK, 0xFB: UNK UNK,
            0xFC: UNK UNK, 0xFD: SBC ABX, 0xFE: INC ABX, 0xFF: UNK UNK,
        }
    };
}

impl Cpu {
    pub fn tick(&mut self) {
        self.world += 1;

        while self.counter < self.world {
            let nmi_cur = self.wires.nmi.get();
            let nmi_prev = self.nmi_prev;
            self.nmi_prev = nmi_cur;

            if nmi_prev && !nmi_cur {
                self.exec_interrupt(Interrupt::Nmi, false);
                continue;
            }

            if self.wires.irq.get() {
                self.exec_interrupt(Interrupt::Rst, false);
                continue;
            }

            if self.wires.rst.get() {
                self.exec_interrupt(Interrupt::Irq, false);
                continue;
            }

            self.exec_one();
        }
    }

    fn exec_one(&mut self) {
        self.trace();

        let opc = self.fetch_u8();

        macro_rules! gen_code {
            ($($opc:literal: $mne:ident $mode:ident, )*) => {
                match opc {
                    $(
                        $opc => exec!($mne, $mode),
                    )*
                }
            };
        }

        macro_rules! exec {
            ($mne:ident, IMP) => {
                exec_op!($mne)
            };
            ($mne:ident, ACC) => {
                exec_op!($mne, ACC)
            };

            ($mne:ident, $mode:ident) => {{
                let addr = effaddr!($mode);
                exec_op!($mne, addr)
            }};
        }

        macro_rules! effaddr {
            (IMM) => {{
                let ret = self.reg.pc;
                self.reg.pc = self.reg.pc.wrapping_add(1);
                ret
            }};
            (ABS) => {{
                self.fetch_u16()
            }};
            (ABX) => {{
                self.fetch_u16().wrapping_add(self.reg.x as u16)
            }};
            (ABY) => {{
                self.fetch_u16().wrapping_add(self.reg.y as u16)
            }};
            (IND) => {{
                let lo = self.fetch_u16();
                let hi = (lo & 0xff00) | (lo as u8).wrapping_add(1) as u16;
                self.read_u8(lo) as u16 | (self.read_u8(hi) as u16) << 8
            }};
            (ZPG) => {{
                self.fetch_u8() as u16
            }};
            (ZPX) => {{
                self.fetch_u8().wrapping_add(self.reg.x) as u16
            }};
            (ZPY) => {{
                self.fetch_u8().wrapping_add(self.reg.y) as u16
            }};
            (INX) => {{
                let a = self.fetch_u8().wrapping_add(self.reg.x);
                let lo = self.read_u8(a as u16);
                let hi = self.read_u8(a.wrapping_add(1) as u16);
                lo as u16 | (hi as u16) << 8
            }};
            (INY) => {{
                let a = self.fetch_u8();
                let lo = self.read_u8(a as u16);
                let hi = self.read_u8(a.wrapping_add(1) as u16);
                (lo as u16 | (hi as u16) << 8) + self.reg.y as u16
            }};
            (REL) => {{
                let rel = self.fetch_u8() as i8;
                self.reg.pc.wrapping_add(rel as u16)
            }};
            (UNK) => {{
                log::warn!("invalid addressing mode");
            }};
        }

        macro_rules! exec_op {
            (ADC, $addr:ident) => {{
                let a = self.reg.a as u16;
                let b = self.read_u8($addr) as u16;
                let c = self.reg.flag.c as u16;
                let r = a.wrapping_add(b).wrapping_add(c);
                self.reg.flag.c = r > 0xff;
                self.reg.flag.v = (a ^ r) & (b ^ r) & 0x80 != 0;
                self.reg.a = r as u8;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (SBC, $addr:ident) => {{
                let a = self.reg.a as u16;
                let b = self.read_u8($addr) as u16;
                let c = self.reg.flag.c as u16;
                let r = a.wrapping_sub(b).wrapping_sub(1 - c);
                self.reg.flag.c = r <= 0xff;
                self.reg.flag.v = (a ^ b) & (a ^ r) & 0x80 != 0;
                self.reg.a = r as u8;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (AND, $addr:ident) => {{
                self.reg.a &= self.read_u8($addr);
                self.reg.flag.set_nz(self.reg.a);
            }};
            (ORA, $addr:ident) => {{
                self.reg.a |= self.read_u8($addr);
                self.reg.flag.set_nz(self.reg.a);
            }};
            (EOR, $addr:ident) => {{
                self.reg.a ^= self.read_u8($addr);
                self.reg.flag.set_nz(self.reg.a);
            }};
            (BIT, $addr:ident) => {{
                let r = self.read_u8($addr);
                self.reg.flag.v = r & 0x40 != 0;
                self.reg.flag.n = r & 0x80 != 0;
                self.reg.flag.z = (self.reg.a & r) == 0;
            }};

            (cmp, $reg:ident, $addr:ident) => {{
                let a = self.reg.$reg as u16;
                let b = self.read_u8($addr) as u16;
                let r = a.wrapping_sub(b);
                self.reg.flag.c = r <= 0xff;
                self.reg.flag.set_nz(r as u8);
            }};
            (CMP, $addr:ident) => {{
                exec_op!(cmp, a, $addr);
            }};
            (CPX, $addr:ident) => {{
                exec_op!(cmp, x, $addr);
            }};
            (CPY, $addr:ident) => {{
                exec_op!(cmp, y, $addr);
            }};

            (ld, $reg:ident, $addr:ident) => {{
                self.reg.$reg = self.read_u8($addr);
                self.reg.flag.set_nz(self.reg.$reg);
            }};
            (LDA, $addr:ident) => {{
                exec_op!(ld, a, $addr)
            }};
            (LDX, $addr:ident) => {{
                exec_op!(ld, x, $addr)
            }};
            (LDY, $addr:ident) => {{
                exec_op!(ld, y, $addr)
            }};

            (st, $reg:ident, $addr:ident) => {{
                self.write_u8($addr, self.reg.$reg);
            }};
            (STA, $addr:ident) => {{
                exec_op!(st, a, $addr)
            }};
            (STX, $addr:ident) => {{
                exec_op!(st, x, $addr)
            }};
            (STY, $addr:ident) => {{
                exec_op!(st, y, $addr)
            }};

            (mov, s, $src:ident) => {{
                self.reg.s = self.reg.$src;
            }};
            (mov, $dest:ident, $src:ident) => {{
                self.reg.$dest = self.reg.$src;
                self.reg.flag.set_nz(self.reg.$dest);
            }};
            (TAX) => {{
                exec_op!(mov, x, a);
            }};
            (TAY) => {{
                exec_op!(mov, y, a);
            }};
            (TXA) => {{
                exec_op!(mov, a, x);
            }};
            (TYA) => {{
                exec_op!(mov, a, y);
            }};
            (TSX) => {{
                exec_op!(mov, x, s);
            }};
            (TXS) => {{
                exec_op!(mov, s, x);
            }};

            (rmw, $op:ident, $addr:ident) => {{
                let mut a = self.read_u8($addr);
                exec_op!($op, a);
                self.write_u8($addr, a);
            }};

            (asl, $var:expr) => {{
                self.reg.flag.c = $var & 0x80 != 0;
                $var <<= 1;
                self.reg.flag.set_nz($var);
            }};
            (lsr, $var:expr) => {{
                self.reg.flag.c = $var & 1 != 0;
                $var >>= 1;
                self.reg.flag.set_nz($var);
            }};
            (rol, $var:expr) => {{
                let t = $var;
                $var = (t << 1) | self.reg.flag.c as u8;
                self.reg.flag.c = t & 0x80 != 0;
                self.reg.flag.set_nz($var);
            }};
            (ror, $var:expr) => {{
                let t = $var;
                $var = (t >> 1) | (self.reg.flag.c as u8) << 7;
                self.reg.flag.c = t & 1 != 0;
                self.reg.flag.set_nz($var);
            }};
            (inc, $var:expr) => {{
                $var = $var.wrapping_add(1);
                self.reg.flag.set_nz($var);
            }};
            (dec, $var:expr) => {{
                $var = $var.wrapping_sub(1);
                self.reg.flag.set_nz($var);
            }};

            (ASL, ACC) => {{
                exec_op!(asl, self.reg.a);
            }};
            (ASL, $addr:ident) => {{
                exec_op!(rmw, asl, $addr);
            }};
            (LSR, ACC) => {{
                exec_op!(lsr, self.reg.a);
            }};
            (LSR, $addr:ident) => {{
                exec_op!(rmw, lsr, $addr);
            }};
            (ROL, ACC) => {{
                exec_op!(rol, self.reg.a);
            }};
            (ROL, $addr:ident) => {{
                exec_op!(rmw, rol, $addr);
            }};
            (ROR, ACC) => {{
                exec_op!(ror, self.reg.a);
            }};
            (ROR, $addr:ident) => {{
                exec_op!(rmw, ror, $addr);
            }};
            (INX) => {{
                exec_op!(inc, self.reg.x);
            }};
            (INY) => {{
                exec_op!(inc, self.reg.y);
            }};
            (INC, $addr:ident) => {{
                exec_op!(rmw, inc, $addr);
            }};
            (DEX) => {{
                exec_op!(dec, self.reg.x);
            }};
            (DEY) => {{
                exec_op!(dec, self.reg.y);
            }};
            (DEC, $addr:ident) => {{
                exec_op!(rmw, dec, $addr);
            }};

            (JMP, $addr:ident) => {{
                self.reg.pc = $addr;
            }};
            (JSR, $addr:ident) => {{
                self.push_u16(self.reg.pc.wrapping_sub(1));
                self.reg.pc = $addr;
            }};
            (RTS) => {{
                self.reg.pc = self.pop_u16().wrapping_add(1)
            }};
            (RTI) => {{
                let p = self.pop_u8();
                self.reg.flag.set_u8(p);
                self.reg.pc = self.pop_u16()
            }};

            (bra, $cond:ident, $val:expr, $addr:ident) => {{
                if self.reg.flag.$cond == $val {
                    self.reg.pc = $addr;
                }
            }};
            (BCC, $addr:ident) => {{
                exec_op!(bra, c, false, $addr)
            }};
            (BCS, $addr:ident) => {{
                exec_op!(bra, c, true, $addr)
            }};
            (BNE, $addr:ident) => {{
                exec_op!(bra, z, false, $addr)
            }};
            (BEQ, $addr:ident) => {{
                exec_op!(bra, z, true, $addr)
            }};
            (BPL, $addr:ident) => {{
                exec_op!(bra, n, false, $addr)
            }};
            (BMI, $addr:ident) => {{
                exec_op!(bra, n, true, $addr)
            }};
            (BVC, $addr:ident) => {{
                exec_op!(bra, v, false, $addr)
            }};
            (BVS, $addr:ident) => {{
                exec_op!(bra, v, true, $addr)
            }};

            (SEC) => {{
                self.reg.flag.c = true;
            }};
            (SED) => {{
                self.reg.flag.d = true;
            }};
            (SEI) => {{
                self.reg.flag.i = true;
            }};
            (CLC) => {{
                self.reg.flag.c = false;
            }};
            (CLD) => {{
                self.reg.flag.d = false;
            }};
            (CLI) => {{
                self.reg.flag.i = false;
            }};
            (CLV) => {{
                self.reg.flag.v = false;
            }};

            (PHA) => {{
                self.push_u8(self.reg.a);
            }};
            (PHP) => {{
                self.push_u8(self.reg.flag.get_u8(3));
            }};
            (PLA) => {{
                self.reg.a = self.pop_u8();
                self.reg.flag.set_nz(self.reg.a);
            }};
            (PLP) => {{
                let p = self.pop_u8();
                self.reg.flag.set_u8(p);
            }};

            (BRK, $addr:ident) => {{
                self.reg.pc = self.reg.pc.wrapping_add(1);
                self.exec_interrupt(Interrupt::Irq, true);
            }};

            (NOP) => {{}};

            (UNK, $addr:ident) => {{
                log::warn!("invalid opcode: ${opc:02X}");
            }};
        }

        instructions!(gen_code);
    }

    fn trace(&self) {
        if !log::log_enabled!(target: "disasm", log::Level::Trace)
            && !log::log_enabled!(target: "disasm-nestest", log::Level::Trace)
        {
            return;
        }

        let pc = self.reg.pc;
        let opc = self.mem.borrow().read(pc);
        let opr =
            self.mem.borrow().read(pc + 1) as u16 | (self.mem.borrow().read(pc + 2) as u16) << 8;

        let asm = disasm(pc, opc, opr);

        log::trace!(target: "disasm",
            "{pc:04X}: {asm:13} | A={a:02X} X={x:02X} Y={y:02X} S={s:02X} {n}{v}{d}{i}{z}{c}",
            pc = self.reg.pc,
            a = self.reg.a,
            x = self.reg.x,
            y = self.reg.y,
            s = self.reg.s,
            n = if self.reg.flag.n { 'N' } else { '-' },
            v = if self.reg.flag.v { 'V' } else { '-' },
            d = if self.reg.flag.d { 'D' } else { '-' },
            i = if self.reg.flag.i { 'I' } else { '-' },
            z = if self.reg.flag.z { 'Z' } else { '-' },
            c = if self.reg.flag.c { 'C' } else { '-' },
        );

        let bytes = match INSTR_TABLE[opc as usize].1.len() {
            1 => format!("{opc:02X}"),
            2 => format!("{opc:02X} {:02X}", opr & 0xff),
            3 => format!("{opc:02X} {:02X} {:02X}", opr & 0xff, opr >> 8),
            _ => unreachable!(),
        };

        let read_u8 = |addr: u16| {
            if addr < 0x2000 || addr >= 0x8000 {
                format!("{:02X}", self.mem.borrow().read(addr))
            } else {
                format!("??")
            }
        };

        let ctx = match &INSTR_TABLE[opc as usize].1 {
            AddrMode::ZPG => format!(" = {}", read_u8(opr & 0xff)),
            AddrMode::ABS => {
                if !matches!(INSTR_TABLE[opc as usize].0, "JMP" | "JSR") {
                    format!(" = {}", read_u8(opr))
                } else {
                    "".to_string()
                }
            }
            AddrMode::IND => format!(
                " = {}{}",
                read_u8((opr & 0xff00) | (opr as u8).wrapping_add(1) as u16),
                read_u8(opr)
            ),
            AddrMode::ZPX => {
                let addr = (opr as u8).wrapping_add(self.reg.x);
                format!(" @ {addr:02X} = {}", read_u8(addr as u16))
            }
            AddrMode::ZPY => {
                let addr = (opr as u8).wrapping_add(self.reg.y);
                format!(" @ {addr:02X} = {}", read_u8(addr as u16))
            }
            AddrMode::ABX => {
                let addr = opr.wrapping_add(self.reg.x as u16);
                format!(" @ {addr:04X} = {}", read_u8(addr as u16))
            }
            AddrMode::ABY => {
                let addr = opr.wrapping_add(self.reg.y as u16);
                format!(" @ {addr:04X} = {}", read_u8(addr as u16))
            }
            AddrMode::INX => {
                let addr = (opr as u8).wrapping_add(self.reg.x);
                let ind = self.mem.borrow().read(addr as u16) as u16
                    | (self.mem.borrow().read(addr.wrapping_add(1) as u16) as u16) << 8;
                format!(" @ {addr:02X} = {ind:04X} = {}", read_u8(ind))
            }
            AddrMode::INY => {
                let ind = self.mem.borrow().read((opr as u8) as u16) as u16
                    | (self.mem.borrow().read((opr as u8).wrapping_add(1) as u16) as u16) << 8;
                let addr = ind + self.reg.y as u16;
                format!(" = {ind:04X} @ {addr:04X} = {}", read_u8(addr))
            }

            AddrMode::IMP | AddrMode::ACC | AddrMode::IMM | AddrMode::REL | AddrMode::UNK => {
                "".to_string()
            }
        };

        let asm = format!("{}{}", asm, ctx);

        log::trace!(target: "disasm-nestest",
            "{pc:04X}  {bytes:8}  {asm:30}  A:{a:02X} X:{x:02X} Y:{y:02X} P:{p:02X} SP:{s:02X}",
            pc = self.reg.pc,
            a = self.reg.a,
            x = self.reg.x,
            y = self.reg.y,
            s = self.reg.s,
            p = self.reg.flag.get_u8(2),
        );
    }
}

macro_rules! instr_table {
    ($($opc:literal: $mne:ident $addr_mode:ident,)*) => {{
        [$(
            (stringify!($mne), AddrMode::$addr_mode),
        )*]
    }}
}

const INSTR_TABLE: [(&str, AddrMode); 256] = instructions!(instr_table);

fn disasm(pc: u16, opc: u8, opr: u16) -> String {
    let opc = opc as usize;
    let (mne, addr_mode) = &INSTR_TABLE[opc];

    match addr_mode {
        AddrMode::IMP => mne.to_string(),
        AddrMode::IMM => format!("{mne} #${:02X}", opr & 0xff),
        AddrMode::ACC => format!("{mne} A"),
        AddrMode::ABS => format!("{mne} ${opr:04X}"),
        AddrMode::ABX => format!("{mne} ${opr:04X},X"),
        AddrMode::ABY => format!("{mne} ${opr:04X},Y"),
        AddrMode::IND => format!("{mne} (${opr:04X})"),
        AddrMode::ZPG => format!("{mne} ${:02X}", opr & 0xff),
        AddrMode::ZPX => format!("{mne} ${:02X},X", opr & 0xff),
        AddrMode::ZPY => format!("{mne} ${:02X},Y", opr & 0xff),
        AddrMode::INX => format!("{mne} (${:02X},X)", opr & 0xff),
        AddrMode::INY => format!("{mne} (${:02X}),Y", opr & 0xff),
        AddrMode::REL => {
            let addr = pc.wrapping_add((opr & 0xff) as i8 as u16).wrapping_add(2);
            format!("{mne} ${:04X}", addr)
        }
        AddrMode::UNK => format!("{mne} ???"),
    }
}
