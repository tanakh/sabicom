use crate::{
    consts::{LINES_PER_FRAME, PPU_CLOCK_PER_LINE},
    memory::MemoryMap,
    util::{Ref, Wire},
};

const NMI_VECTOR: u16 = 0xFFFA;
const RST_VECTOR: u16 = 0xFFFC;
const IRQ_VECTOR: u16 = 0xFFFE;

pub struct Cpu {
    world: u64,
    counter: u64,

    pub reg: Register,

    mem: Ref<MemoryMap>,
    wires: Wires,

    nmi_prev: bool,
    i_flag_prev: bool,
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

pub struct Register {
    a: u8,
    x: u8,
    y: u8,
    s: u8,
    pub pc: u16,
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
            counter: 2,
            world: 0,
            reg: Register::new(),
            wires,
            nmi_prev: false,
            i_flag_prev: false,
        };
        ret.exec_interrupt(Interrupt::Rst, false);
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
        self.reg.pc = self.read(vect) as u16 | (self.read(vect + 1) as u16) << 8;
        self.reg.flag.i = true;
    }

    fn read(&mut self, addr: u16) -> u8 {
        let ret = self.mem.borrow().read(addr);
        self.tick_bus();
        log::trace!(target: "prgmem", "[${addr:04X}] -> ${ret:02X}");
        ret
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.mem.borrow_mut().write(addr, data);
        self.tick_bus();
        log::trace!(target: "prgmem", "[${addr:04X}] <- ${data:02X}");
    }

    fn fetch_u8(&mut self) -> u8 {
        let ret = self.read(self.reg.pc);
        self.reg.pc = self.reg.pc.wrapping_add(1);
        ret
    }

    fn fetch_u16(&mut self) -> u16 {
        let lo = self.fetch_u8();
        let hi = self.fetch_u8();
        lo as u16 | (hi as u16) << 8
    }

    fn push_u8(&mut self, data: u8) {
        self.write(0x100 + self.reg.s as u16, data);
        self.reg.s = self.reg.s.wrapping_sub(1);
    }

    fn push_u16(&mut self, data: u16) {
        self.push_u8((data >> 8) as u8);
        self.push_u8(data as u8);
    }

    fn pop_u8(&mut self) -> u8 {
        self.reg.s = self.reg.s.wrapping_add(1);
        self.read(0x100 + self.reg.s as u16)
    }

    fn pop_u16(&mut self) -> u16 {
        let lo = self.pop_u8() as u16;
        let hi = self.pop_u8() as u16;
        lo | (hi << 8)
    }
}

#[allow(clippy::upper_case_acronyms)]
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
            0x00: BRK IMP, 0x01: ORA INX, 0x02: UNK UNK, 0x03:*SLO INX,
            0x04:*NOP ZPG, 0x05: ORA ZPG, 0x06: ASL ZPG, 0x07:*SLO ZPG,
            0x08: PHP IMP, 0x09: ORA IMM, 0x0A: ASL ACC, 0x0B:*AAC IMM,
            0x0C:*NOP ABS, 0x0D: ORA ABS, 0x0E: ASL ABS, 0x0F:*SLO ABS,
            0x10: BPL REL, 0x11: ORA INY, 0x12: UNK UNK, 0x13:*SLO INY,
            0x14:*NOP ZPX, 0x15: ORA ZPX, 0x16: ASL ZPX, 0x17:*SLO ZPX,
            0x18: CLC IMP, 0x19: ORA ABY, 0x1A:*NOP IMP, 0x1B:*SLO ABY,
            0x1C:*NOP ABX, 0x1D: ORA ABX, 0x1E: ASL ABX, 0x1F:*SLO ABX,
            0x20: JSR ABS, 0x21: AND INX, 0x22: UNK UNK, 0x23:*RLA INX,
            0x24: BIT ZPG, 0x25: AND ZPG, 0x26: ROL ZPG, 0x27:*RLA ZPG,
            0x28: PLP IMP, 0x29: AND IMM, 0x2A: ROL ACC, 0x2B:*AAC IMM,
            0x2C: BIT ABS, 0x2D: AND ABS, 0x2E: ROL ABS, 0x2F:*RLA ABS,
            0x30: BMI REL, 0x31: AND INY, 0x32: UNK UNK, 0x33:*RLA INY,
            0x34:*NOP ZPX, 0x35: AND ZPX, 0x36: ROL ZPX, 0x37:*RLA ZPX,
            0x38: SEC IMP, 0x39: AND ABY, 0x3A:*NOP IMP, 0x3B:*RLA ABY,
            0x3C:*NOP ABX, 0x3D: AND ABX, 0x3E: ROL ABX, 0x3F:*RLA ABX,
            0x40: RTI IMP, 0x41: EOR INX, 0x42: UNK UNK, 0x43:*SRE INX,
            0x44:*NOP ZPG, 0x45: EOR ZPG, 0x46: LSR ZPG, 0x47:*SRE ZPG,
            0x48: PHA IMP, 0x49: EOR IMM, 0x4A: LSR ACC, 0x4B:*ASR IMM,
            0x4C: JMP ABS, 0x4D: EOR ABS, 0x4E: LSR ABS, 0x4F:*SRE ABS,
            0x50: BVC REL, 0x51: EOR INY, 0x52: UNK UNK, 0x53:*SRE INY,
            0x54:*NOP ZPX, 0x55: EOR ZPX, 0x56: LSR ZPX, 0x57:*SRE ZPX,
            0x58: CLI IMP, 0x59: EOR ABY, 0x5A:*NOP IMP, 0x5B:*SRE ABY,
            0x5C:*NOP ABX, 0x5D: EOR ABX, 0x5E: LSR ABX, 0x5F:*SRE ABX,
            0x60: RTS IMP, 0x61: ADC INX, 0x62: UNK UNK, 0x63:*RRA INX,
            0x64:*NOP ZPG, 0x65: ADC ZPG, 0x66: ROR ZPG, 0x67:*RRA ZPG,
            0x68: PLA IMP, 0x69: ADC IMM, 0x6A: ROR ACC, 0x6B:*ARR IMM,
            0x6C: JMP IND, 0x6D: ADC ABS, 0x6E: ROR ABS, 0x6F:*RRA ABS,
            0x70: BVS REL, 0x71: ADC INY, 0x72: UNK UNK, 0x73:*RRA INY,
            0x74:*NOP ZPX, 0x75: ADC ZPX, 0x76: ROR ZPX, 0x77:*RRA ZPX,
            0x78: SEI IMP, 0x79: ADC ABY, 0x7A:*NOP IMP, 0x7B:*RRA ABY,
            0x7C:*NOP ABX, 0x7D: ADC ABX, 0x7E: ROR ABX, 0x7F:*RRA ABX,
            0x80:*NOP IMM, 0x81: STA INX, 0x82:*NOP IMM, 0x83:*SAX INX,
            0x84: STY ZPG, 0x85: STA ZPG, 0x86: STX ZPG, 0x87:*SAX ZPG,
            0x88: DEY IMP, 0x89:*NOP IMM, 0x8A: TXA IMP, 0x8B: UNK UNK,
            0x8C: STY ABS, 0x8D: STA ABS, 0x8E: STX ABS, 0x8F:*SAX ABS,
            0x90: BCC REL, 0x91: STA INY, 0x92: UNK UNK, 0x93: UNK UNK,
            0x94: STY ZPX, 0x95: STA ZPX, 0x96: STX ZPY, 0x97:*SAX ZPY,
            0x98: TYA IMP, 0x99: STA ABY, 0x9A: TXS IMP, 0x9B: UNK UNK,
            0x9C:*SYA ABX, 0x9D: STA ABX, 0x9E:*SXA ABY, 0x9F: UNK UNK,
            0xA0: LDY IMM, 0xA1: LDA INX, 0xA2: LDX IMM, 0xA3:*LAX INX,
            0xA4: LDY ZPG, 0xA5: LDA ZPG, 0xA6: LDX ZPG, 0xA7:*LAX ZPG,
            0xA8: TAY IMP, 0xA9: LDA IMM, 0xAA: TAX IMP, 0xAB:*ATX IMM,
            0xAC: LDY ABS, 0xAD: LDA ABS, 0xAE: LDX ABS, 0xAF:*LAX ABS,
            0xB0: BCS REL, 0xB1: LDA INY, 0xB2: UNK UNK, 0xB3:*LAX INY,
            0xB4: LDY ZPX, 0xB5: LDA ZPX, 0xB6: LDX ZPY, 0xB7:*LAX ZPY,
            0xB8: CLV IMP, 0xB9: LDA ABY, 0xBA: TSX IMP, 0xBB: UNK UNK,
            0xBC: LDY ABX, 0xBD: LDA ABX, 0xBE: LDX ABY, 0xBF:*LAX ABY,
            0xC0: CPY IMM, 0xC1: CMP INX, 0xC2:*NOP IMM, 0xC3:*DCP INX,
            0xC4: CPY ZPG, 0xC5: CMP ZPG, 0xC6: DEC ZPG, 0xC7:*DCP ZPG,
            0xC8: INY IMP, 0xC9: CMP IMM, 0xCA: DEX IMP, 0xCB:*AXS IMM,
            0xCC: CPY ABS, 0xCD: CMP ABS, 0xCE: DEC ABS, 0xCF:*DCP ABS,
            0xD0: BNE REL, 0xD1: CMP INY, 0xD2: UNK UNK, 0xD3:*DCP INY,
            0xD4:*NOP ZPX, 0xD5: CMP ZPX, 0xD6: DEC ZPX, 0xD7:*DCP ZPX,
            0xD8: CLD IMP, 0xD9: CMP ABY, 0xDA:*NOP IMP, 0xDB:*DCP ABY,
            0xDC:*NOP ABX, 0xDD: CMP ABX, 0xDE: DEC ABX, 0xDF:*DCP ABX,
            0xE0: CPX IMM, 0xE1: SBC INX, 0xE2:*NOP IMM, 0xE3:*ISB INX,
            0xE4: CPX ZPG, 0xE5: SBC ZPG, 0xE6: INC ZPG, 0xE7:*ISB ZPG,
            0xE8: INX IMP, 0xE9: SBC IMM, 0xEA: NOP IMP, 0xEB:*SBC IMM,
            0xEC: CPX ABS, 0xED: SBC ABS, 0xEE: INC ABS, 0xEF:*ISB ABS,
            0xF0: BEQ REL, 0xF1: SBC INY, 0xF2: UNK UNK, 0xF3:*ISB INY,
            0xF4:*NOP ZPX, 0xF5: SBC ZPX, 0xF6: INC ZPX, 0xF7:*ISB ZPX,
            0xF8: SED IMP, 0xF9: SBC ABY, 0xFA:*NOP IMP, 0xFB:*ISB ABY,
            0xFC:*NOP ABX, 0xFD: SBC ABX, 0xFE: INC ABX, 0xFF:*ISB ABX,
        }
    };
}

impl Cpu {
    pub fn tick(&mut self) {
        let stall = self.mem.borrow().cpu_stall;
        if stall > 0 {
            self.mem.borrow_mut().cpu_stall = 0;
            for _ in 0..stall {
                self.tick_bus();
            }
        }

        self.world += 1;

        while self.counter < self.world {
            let nmi_cur = self.wires.nmi.get();
            let nmi_prev = self.nmi_prev;
            self.nmi_prev = nmi_cur;

            let irq_prev = self.wires.irq.get();
            self.i_flag_prev = self.reg.flag.i;

            self.exec_one();

            if nmi_prev && !nmi_cur {
                self.exec_interrupt(Interrupt::Nmi, false);
                continue;
            }

            if !self.i_flag_prev && irq_prev {
                self.exec_interrupt(Interrupt::Irq, false);
                continue;
            }
        }
    }

    fn tick_bus(&mut self) {
        self.counter += 1;
        self.mem.borrow_mut().tick();
    }

    fn exec_one(&mut self) {
        self.trace();

        let opaddr = self.reg.pc;
        let opc = self.fetch_u8();

        macro_rules! gen_code {
            ($($opc:literal: $a:tt $b:ident $($c:ident)?, )*) => {{
                match opc {
                    $( $opc => exec!($a $b $($c)*), )*
                }
            }};
        }

        macro_rules! is_read {
            (STA) => {
                false
            };
            (LSR) => {
                false
            };
            (ASL) => {
                false
            };
            (ROR) => {
                false
            };
            (ROL) => {
                false
            };
            (INC) => {
                false
            };
            (DEC) => {
                false
            };
            ($mne:ident) => {
                true
            };
        }

        macro_rules! exec {
            (*$mne:ident $mode:ident) => {
                exec!($mne $mode)
            };
            ($mne:ident IMP) => {{
                let _ = self.read(self.reg.pc);
                exec_op!($mne)
            }};
            ($mne:ident ACC) => {{
                let _ = self.read(self.reg.pc);
                exec_op!($mne, ACC)
            }};


            ($mne:ident $mode:ident) => {{
                #[allow(unused_variables)]
                let read = is_read!($mne);
                #[allow(unused_variables)]
                let addr = effaddr!($mode, read);
                exec_op!($mne, addr)
            }};
        }

        macro_rules! effaddr {
            (IMM, $read:ident) => {{
                let ret = self.reg.pc;
                self.reg.pc = self.reg.pc.wrapping_add(1);
                ret
            }};
            (ABS, $read:ident) => {{
                self.fetch_u16()
            }};
            (ABX, $read:ident) => {
                effaddr!(abs_ix, x, $read)
            };
            (ABY, $read:ident) => {
                effaddr!(abs_ix, y, $read)
            };
            (abs_ix, $reg:ident, $read:ident) => {{
                let addr = self.fetch_u16();
                let tmp = (addr & 0xff) + self.reg.$reg as u16;
                if !$read || tmp >= 0x100 {
                    let _ = self.read(addr & 0xff00 | tmp & 0xff);
                }
                addr.wrapping_add(self.reg.$reg as u16)
            }};
            (IND, $read:ident) => {{
                let lo = self.fetch_u16();
                let hi = (lo & 0xff00) | (lo as u8).wrapping_add(1) as u16;
                self.read(lo) as u16 | (self.read(hi) as u16) << 8
            }};
            (ZPG, $read:ident) => {{
                self.fetch_u8() as u16
            }};
            (ZPX, $read:ident) => {{
                let addr = self.fetch_u8();
                self.read(addr as u16);
                addr.wrapping_add(self.reg.x) as u16
            }};
            (ZPY, $read:ident) => {{
                let addr = self.fetch_u8();
                self.read(addr as u16);
                addr.wrapping_add(self.reg.y) as u16
            }};
            (INX, $read:ident) => {{
                let a = self.fetch_u8();
                let _ = self.read(a as u16);
                let a = a.wrapping_add(self.reg.x);
                let lo = self.read(a as u16);
                let hi = self.read(a.wrapping_add(1) as u16);
                lo as u16 | (hi as u16) << 8
            }};
            (INY, $read:ident) => {{
                let a = self.fetch_u8();
                let lo = self.read(a as u16) as u16;
                let hi = self.read(a.wrapping_add(1) as u16) as u16;
                let addr = (lo | hi << 8);
                let tmp = lo + self.reg.y as u16;
                if !$read || tmp >= 0x100 {
                    let _ = self.read(hi << 8 | tmp & 0xff);
                }
                addr.wrapping_add(self.reg.y as u16)
            }};
            (REL, $read:ident) => {{
                let rel = self.fetch_u8() as i8;
                self.reg.pc.wrapping_add(rel as u16)
            }};
            (UNK, $read:ident) => {{}};
        }

        macro_rules! exec_op {
            (ADC, $addr:ident) => {{
                let a = self.reg.a as u16;
                let b = self.read($addr) as u16;
                let c = self.reg.flag.c as u16;
                let r = a.wrapping_add(b).wrapping_add(c);
                self.reg.flag.c = r > 0xff;
                self.reg.flag.v = (a ^ r) & (b ^ r) & 0x80 != 0;
                self.reg.a = r as u8;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (SBC, $addr:ident) => {{
                let a = self.reg.a as u16;
                let b = self.read($addr) as u16;
                let c = self.reg.flag.c as u16;
                let r = a.wrapping_sub(b).wrapping_sub(1 - c);
                self.reg.flag.c = r <= 0xff;
                self.reg.flag.v = (a ^ b) & (a ^ r) & 0x80 != 0;
                self.reg.a = r as u8;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (AND, $addr:ident) => {{
                self.reg.a &= self.read($addr);
                self.reg.flag.set_nz(self.reg.a);
            }};
            (ORA, $addr:ident) => {{
                self.reg.a |= self.read($addr);
                self.reg.flag.set_nz(self.reg.a);
            }};
            (EOR, $addr:ident) => {{
                self.reg.a ^= self.read($addr);
                self.reg.flag.set_nz(self.reg.a);
            }};
            (BIT, $addr:ident) => {{
                let r = self.read($addr);
                self.reg.flag.v = r & 0x40 != 0;
                self.reg.flag.n = r & 0x80 != 0;
                self.reg.flag.z = (self.reg.a & r) == 0;
            }};

            (cmp, $reg:ident, $addr:ident) => {{
                let a = self.reg.$reg as u16;
                let b = self.read($addr) as u16;
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
                self.reg.$reg = self.read($addr);
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
                self.write($addr, self.reg.$reg);
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
                let mut a = self.read($addr);
                self.write($addr, a);
                exec_op!($op, a);
                self.write($addr, a);
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
                let _ = self.read(self.reg.s as u16 | 0x100);
                self.push_u16(self.reg.pc.wrapping_sub(1));
                self.reg.pc = $addr;
            }};
            (RTS) => {{
                let _ = self.read(self.reg.s as u16 | 0x100);
                let pc = self.pop_u16();
                let _ = self.read(pc);
                self.reg.pc = pc.wrapping_add(1);
            }};
            (RTI) => {{
                let _ = self.read(self.reg.s as u16 | 0x100);
                let p = self.pop_u8();
                self.reg.flag.set_u8(p);
                // Flag set by RTI affects interrupts
                self.i_flag_prev = self.reg.flag.i;
                self.reg.pc = self.pop_u16()
            }};

            (bra, $cond:ident, $val:expr, $addr:ident) => {{
                if self.reg.flag.$cond == $val {
                    let _ = self.read(self.reg.pc);
                    if self.reg.pc & 0xff00 != $addr & 0xff00 {
                        self.read(self.reg.pc & 0xff00 | $addr & 0xff);
                    }
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
                let _ = self.read(self.reg.s as u16 | 0x100);
                self.reg.a = self.pop_u8();
                self.reg.flag.set_nz(self.reg.a);
            }};
            (PLP) => {{
                let _ = self.read(self.reg.s as u16 | 0x100);
                let p = self.pop_u8();
                self.reg.flag.set_u8(p);
            }};

            (BRK) => {{
                self.reg.pc = self.reg.pc.wrapping_add(1);
                self.exec_interrupt(Interrupt::Irq, true);
                // Interrupt after BRK did not happen
                self.i_flag_prev = self.reg.flag.i;
            }};

            (NOP) => {{}};

            // Undocumented
            (NOP, $addr:ident) => {{
                let _ = self.read($addr);
            }};

            (LAX, $addr:ident) => {{
                self.reg.a = self.read($addr);
                self.reg.x = self.reg.a;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (SAX, $addr:ident) => {{
                self.write($addr, self.reg.a & self.reg.x);
            }};
            (DCP, $addr:ident) => {{
                let b = self.read($addr);
                self.write($addr, b);
                let b = b.wrapping_sub(1);
                self.write($addr, b);
                let r = (self.reg.a as u16).wrapping_sub(b as u16);
                self.reg.flag.c = r <= 0xff;
                self.reg.flag.set_nz(r as u8);
            }};
            (ISB, $addr:ident) => {{
                let b = self.read($addr);
                self.write($addr, b);
                let b = b.wrapping_add(1);
                self.write($addr, b);
                let a = self.reg.a as u16;
                let b = b as u16;
                let c = self.reg.flag.c as u16;
                let r = a.wrapping_sub(b).wrapping_sub(1 - c);
                self.reg.flag.c = r <= 0xff;
                self.reg.flag.v = (a ^ b) & (a ^ r) & 0x80 != 0;
                self.reg.a = r as u8;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (SLO, $addr:ident) => {{
                let b = self.read($addr);
                self.write($addr, b);
                self.reg.flag.c = b >> 7 != 0;
                let b = b << 1;
                self.reg.a |= b;
                self.write($addr, b);
                self.reg.flag.set_nz(self.reg.a);
            }};
            (RLA, $addr:ident) => {{
                let b = self.read($addr);
                self.write($addr, b);
                let c = self.reg.flag.c;
                self.reg.flag.c = b >> 7 != 0;
                let b = (b << 1) | c as u8;
                self.write($addr, b);
                self.reg.a &= b;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (SRE, $addr:ident) => {{
                let b = self.read($addr);
                self.write($addr, b);
                self.reg.flag.c = b & 1 != 0;
                let b = b >> 1;
                self.write($addr, b);
                self.reg.a ^= b;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (RRA, $addr:ident) => {{
                let b = self.read($addr);
                self.write($addr, b);
                let c = self.reg.flag.c as u8;
                self.reg.flag.c = b & 1 != 0;
                let b = (b >> 1) | (c << 7);
                self.write($addr, b);
                let a = self.reg.a as u16;
                let b = b as u16;
                let c = self.reg.flag.c as u16;
                let r = a.wrapping_add(b).wrapping_add(c);
                self.reg.flag.c = r > 0xff;
                self.reg.flag.v = (a ^ r) & (b ^ r) & 0x80 != 0;
                self.reg.a = r as u8;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (AAC, $addr:ident) => {{
                self.reg.a &= self.read($addr);
                self.reg.flag.set_nz(self.reg.a);
                self.reg.flag.c = self.reg.flag.n;
            }};
            (ASR, $addr:ident) => {{
                self.reg.a &= self.read($addr);
                self.reg.flag.c = self.reg.a & 1 != 0;
                self.reg.a >>= 1;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (ARR, $addr:ident) => {{
                self.reg.a &= self.read($addr);
                self.reg.a = (self.reg.a >> 1) | (self.reg.flag.c as u8) << 7;
                self.reg.flag.set_nz(self.reg.a);
                self.reg.flag.c = (self.reg.a >> 6) & 1 != 0;
                self.reg.flag.v = ((self.reg.a >> 5) & 1 != 0) != self.reg.flag.c;
            }};
            (ATX, $addr:ident) => {{
                self.reg.a = self.read($addr);
                self.reg.x = self.reg.a;
                self.reg.flag.set_nz(self.reg.a);
            }};
            (AXS, $addr:ident) => {{
                let t = ((self.reg.x & self.reg.a) as u16).wrapping_sub(self.read($addr) as u16);
                self.reg.x = t as u8;
                self.reg.flag.set_nz(self.reg.x);
                self.reg.flag.c = t <= 0xff;
            }};
            (SYA, $addr:ident) => {{
                let t = self.reg.y & (($addr >> 8) + 1) as u8;
                if self.reg.x as u16 + self.read(opaddr.wrapping_add(1)) as u16 <= 0xff {
                    self.write($addr, t);
                }
            }};
            (SXA, $addr:ident) => {{
                let t = self.reg.x & (($addr >> 8) + 1) as u8;
                if self.reg.y as u16 + self.read(opaddr.wrapping_add(1)) as u16 <= 0xff {
                    self.write($addr, t);
                }
            }};

            (UNK, $addr:ident) => {{
                log::warn!("invalid opcode: ${opc:02X}");
            }};
        }

        instructions!(gen_code);
    }

    fn trace(&self) {
        if !log::log_enabled!(target: "disasm", log::Level::Trace)
            && !log::log_enabled!(target: "disasnt", log::Level::Trace)
        {
            return;
        }

        let pc = self.reg.pc;
        let opc = self.mem.borrow().read(pc);
        let opr =
            self.mem.borrow().read(pc + 1) as u16 | (self.mem.borrow().read(pc + 2) as u16) << 8;

        let ppu_cycle = self.counter * 3;
        let line = ppu_cycle / PPU_CLOCK_PER_LINE % LINES_PER_FRAME as u64;
        let col = ppu_cycle % PPU_CLOCK_PER_LINE;

        let asm = disasm(pc, opc, opr);
        let prg_page = if pc & 0x8000 != 0 {
            format!(
                "{:02X}",
                self.mem
                    .borrow()
                    .mapper()
                    .get_prg_page(((pc & !0x8000) / 0x2000) as _)
            )
        } else {
            "  ".to_string()
        };

        log::trace!(target: "disasm",
            "{prg_page}:{pc:04X}: {asm:13} | A:{a:02X} X:{x:02X} Y:{y:02X} S:{s:02X} P:{n}{v}{d}{i}{z}{c} PPU:{line:3},{col:3}",
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

        let read = |addr: u16| {
            if addr < 0x2000 || addr >= 0x8000 {
                format!("{:02X}", self.mem.borrow().read(addr))
            } else {
                format!("??")
            }
        };

        let ctx = match &INSTR_TABLE[opc as usize].1 {
            AddrMode::ZPG => format!(" = {}", read(opr & 0xff)),
            AddrMode::ABS => {
                if !matches!(INSTR_TABLE[opc as usize].0, "JMP" | "JSR") {
                    format!(" = {}", read(opr))
                } else {
                    "".to_string()
                }
            }
            AddrMode::IND => format!(
                " = {}{}",
                read((opr & 0xff00) | (opr as u8).wrapping_add(1) as u16),
                read(opr)
            ),
            AddrMode::ZPX => {
                let addr = (opr as u8).wrapping_add(self.reg.x);
                format!(" @ {addr:02X} = {}", read(addr as u16))
            }
            AddrMode::ZPY => {
                let addr = (opr as u8).wrapping_add(self.reg.y);
                format!(" @ {addr:02X} = {}", read(addr as u16))
            }
            AddrMode::ABX => {
                let addr = opr.wrapping_add(self.reg.x as u16);
                format!(" @ {addr:04X} = {}", read(addr as u16))
            }
            AddrMode::ABY => {
                let addr = opr.wrapping_add(self.reg.y as u16);
                format!(" @ {addr:04X} = {}", read(addr as u16))
            }
            AddrMode::INX => {
                let addr = (opr as u8).wrapping_add(self.reg.x);
                let ind = self.mem.borrow().read(addr as u16) as u16
                    | (self.mem.borrow().read(addr.wrapping_add(1) as u16) as u16) << 8;
                format!(" @ {addr:02X} = {ind:04X} = {}", read(ind))
            }
            AddrMode::INY => {
                let ind = self.mem.borrow().read((opr as u8) as u16) as u16
                    | (self.mem.borrow().read((opr as u8).wrapping_add(1) as u16) as u16) << 8;
                let addr = ind.wrapping_add(self.reg.y as u16);
                format!(" = {ind:04X} @ {addr:04X} = {}", read(addr))
            }

            AddrMode::IMP | AddrMode::ACC | AddrMode::IMM | AddrMode::REL | AddrMode::UNK => {
                "".to_string()
            }
        };

        let asm = format!("{}{}", asm, ctx);

        log::trace!(target: "disasnt",
            "{pc:04X}  {bytes:8} {asm:32} \
            A:{a:02X} X:{x:02X} Y:{y:02X} P:{p:02X} SP:{s:02X} \
            PPU:{line:3},{col:3} CYC:{cyc}",
            pc = self.reg.pc,
            a = self.reg.a,
            x = self.reg.x,
            y = self.reg.y,
            s = self.reg.s,
            p = self.reg.flag.get_u8(2),
            cyc = self.counter,
        );
    }
}

macro_rules! instr_table {
    ($($opc:literal: $a:tt $b:ident $($c:ident)?, )*) => {{
        [$(
            instr_entry!($a $b $($c)*),
        )*]
    }};
}

macro_rules! instr_entry {
    (*$mne:ident $mode:ident) => {{
        (stringify!($mne), AddrMode::$mode, false)
    }};
    ($mne:ident $mode:ident) => {{
        (stringify!($mne), AddrMode::$mode, true)
    }};
}

const INSTR_TABLE: [(&str, AddrMode, bool); 256] = instructions!(instr_table);

fn disasm(pc: u16, opc: u8, opr: u16) -> String {
    let opc = opc as usize;
    let (mne, addr_mode, official) = &INSTR_TABLE[opc];
    let u = if *official { ' ' } else { '*' };

    match addr_mode {
        AddrMode::IMP => format!("{u}{mne}"),
        AddrMode::IMM => format!("{u}{mne} #${:02X}", opr & 0xff),
        AddrMode::ACC => format!("{u}{mne} A"),
        AddrMode::ABS => format!("{u}{mne} ${opr:04X}"),
        AddrMode::ABX => format!("{u}{mne} ${opr:04X},X"),
        AddrMode::ABY => format!("{u}{mne} ${opr:04X},Y"),
        AddrMode::IND => format!("{u}{mne} (${opr:04X})"),
        AddrMode::ZPG => format!("{u}{mne} ${:02X}", opr & 0xff),
        AddrMode::ZPX => format!("{u}{mne} ${:02X},X", opr & 0xff),
        AddrMode::ZPY => format!("{u}{mne} ${:02X},Y", opr & 0xff),
        AddrMode::INX => format!("{u}{mne} (${:02X},X)", opr & 0xff),
        AddrMode::INY => format!("{u}{mne} (${:02X}),Y", opr & 0xff),
        AddrMode::REL => {
            let addr = pc.wrapping_add((opr & 0xff) as i8 as u16).wrapping_add(2);
            format!("{u}{mne} ${:04X}", addr)
        }
        AddrMode::UNK => format!("{u}{mne} ???"),
    }
}
