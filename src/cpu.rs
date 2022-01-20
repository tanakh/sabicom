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

    pub fn tick(&mut self) {
        self.world += 1;

        while self.counter < self.world {
            self.exec_one();
        }
    }

    fn exec_one(&mut self) {
        self.trace();

        let nmi_cur = self.wires.nmi.get();
        let nmi_prev = self.nmi_prev;
        self.nmi_prev = nmi_cur;

        if nmi_prev && !nmi_cur {
            self.exec_interrupt(Interrupt::Nmi, false);
            return;
        }

        if self.wires.irq.get() {
            self.exec_interrupt(Interrupt::Rst, false);
            return;
        }

        if self.wires.rst.get() {
            self.exec_interrupt(Interrupt::Irq, false);
            return;
        }

        let opc = self.fetch_u8();

        macro_rules! imm {
            () => {{
                let ret = self.reg.pc;
                self.reg.pc = self.reg.pc.wrapping_add(1);
                ret
            }};
        }

        macro_rules! abs {
            () => {{
                self.fetch_u16()
            }};
        }

        macro_rules! abx {
            () => {{
                self.fetch_u16() + self.reg.x as u16
            }};
        }

        macro_rules! aby {
            () => {{
                self.fetch_u16() + self.reg.y as u16
            }};
        }

        macro_rules! absi {
            () => {{
                let t = self.fetch_u16();
                self.read_u16(t)
            }};
        }

        macro_rules! zp {
            () => {{
                self.fetch_u8() as u16
            }};
        }

        macro_rules! zpx {
            () => {{
                self.fetch_u8().wrapping_add(self.reg.x) as u16
            }};
        }

        macro_rules! zpy {
            () => {{
                self.fetch_u8().wrapping_add(self.reg.y) as u16
            }};
        }

        macro_rules! zpxi {
            () => {{
                let a = self.fetch_u8().wrapping_add(self.reg.x);
                let lo = self.read_u8(a as u16);
                let hi = self.read_u8(a.wrapping_add(1) as u16);
                lo as u16 | (hi as u16) << 8
            }};
        }

        macro_rules! zpiy {
            () => {{
                let a = self.fetch_u8();
                let lo = self.read_u8(a as u16);
                let hi = self.read_u8(a.wrapping_add(1) as u16);
                (lo as u16 | (hi as u16) << 8) + self.reg.y as u16
            }};
        }

        macro_rules! adc {
            ($addrmode:expr) => {{
                let addr = $addrmode;
                let a = self.reg.a as u16;
                let b = self.read_u8(addr) as u16;
                let c = self.reg.flag.c as u16;
                let r = a.wrapping_add(b).wrapping_add(c);
                self.reg.flag.c = r > 0xff;
                self.reg.flag.v = (a ^ r) & (b ^ r) & 0x80 != 0;
                self.reg.a = r as u8;
                self.reg.flag.set_nz(self.reg.a);
            }};
        }

        macro_rules! sbc {
            ($addrmode:expr) => {{
                let addr = $addrmode;
                let a = self.reg.a as u16;
                let b = self.read_u8(addr) as u16;
                let c = self.reg.flag.c as u16;
                let r = a.wrapping_sub(b).wrapping_sub(1 - c);
                self.reg.flag.c = r <= 0xff;
                self.reg.flag.v = (a ^ b) & (a ^ r) & 0x80 != 0;
                self.reg.a = r as u8;
                self.reg.flag.set_nz(self.reg.a);
            }};
        }

        macro_rules! cmp {
            ($reg:ident, $addrmode:expr) => {{
                let addr = $addrmode;
                let a = self.reg.$reg as u16;
                let b = self.read_u8(addr) as u16;
                let r = a.wrapping_sub(b);
                self.reg.flag.c = r <= 0xff;
                self.reg.flag.set_nz(r as u8);
            }};
        }

        macro_rules! and {
            ($addrmode:expr) => {{
                let addr = $addrmode;
                self.reg.a &= self.read_u8(addr);
                self.reg.flag.set_nz(self.reg.a);
            }};
        }

        macro_rules! ora {
            ($addrmode:expr) => {{
                let addr = $addrmode;
                self.reg.a |= self.read_u8(addr);
                self.reg.flag.set_nz(self.reg.a);
            }};
        }

        macro_rules! eor {
            ($addrmode:expr) => {{
                let addr = $addrmode;
                self.reg.a ^= self.read_u8(addr);
                self.reg.flag.set_nz(self.reg.a);
            }};
        }

        macro_rules! bit {
            ($addrmode:expr) => {{
                let addr = $addrmode;
                let r = self.read_u8(addr);
                self.reg.flag.v = r & 0x40 != 0;
                self.reg.flag.n = r & 0x80 != 0;
                self.reg.flag.z = (self.reg.a & r) == 0;
            }};
        }

        macro_rules! load {
            ($reg:ident, $addrmode:expr) => {{
                let addr = $addrmode;
                self.reg.$reg = self.read_u8(addr);
                self.reg.flag.set_nz(self.reg.$reg);
            }};
        }

        macro_rules! store {
            ($reg:ident, $addrmode:expr) => {{
                let addr = $addrmode;
                self.write_u8(addr, self.reg.$reg);
            }};
        }

        macro_rules! mov {
            ($dest:ident, $src:ident) => {{
                self.reg.$dest = self.reg.$src;
                self.reg.flag.set_nz(self.reg.$dest);
            }};
            ($dest:ident, $src:ident, false) => {{
                self.reg.$dest = self.reg.$src;
            }};
        }

        macro_rules! modify {
            ($op:ident, $reg:ident) => {{
                $op!(self.reg.$reg);
            }};
        }

        macro_rules! read_modify_write {
            ($op:ident, $addrmode:expr) => {{
                let addr = $addrmode;
                let mut t = self.read_u8(addr);
                $op!(t);
                self.write_u8(addr, t);
            }};
        }

        macro_rules! asl {
            ($var:expr) => {{
                self.reg.flag.c = $var & 0x80 != 0;
                $var <<= 1;
                self.reg.flag.set_nz($var);
            }};
        }

        macro_rules! lsr {
            ($var:expr) => {{
                self.reg.flag.c = $var & 1 != 0;
                $var >>= 1;
                self.reg.flag.set_nz($var);
            }};
        }

        macro_rules! rol {
            ($var:expr) => {{
                let t = $var;
                $var = (t << 1) | self.reg.flag.c as u8;
                self.reg.flag.c = t & 0x80 != 0;
                self.reg.flag.set_nz($var);
            }};
        }

        macro_rules! ror {
            ($var:expr) => {{
                let t = $var;
                $var = (t >> 1) | (self.reg.flag.c as u8) << 7;
                self.reg.flag.c = t & 1 != 0;
                self.reg.flag.set_nz($var);
            }};
        }

        macro_rules! inc {
            ($var:expr) => {{
                $var = $var.wrapping_add(1);
                self.reg.flag.set_nz($var);
            }};
        }

        macro_rules! dec {
            ($var:expr) => {{
                $var = $var.wrapping_sub(1);
                self.reg.flag.set_nz($var);
            }};
        }

        macro_rules! bra {
            ($cond:ident, $val:expr) => {{
                let rel = self.fetch_u8() as i8;
                if self.reg.flag.$cond == $val {
                    // TODO: accurate cycle count
                    self.counter += 1;
                    self.reg.pc = self.reg.pc.wrapping_add(rel as u16);
                }
            }};
        }

        match opc {
            0x69 => adc!(imm!()),
            0x65 => adc!(zp!()),
            0x75 => adc!(zpx!()),
            0x6D => adc!(abs!()),
            0x7D => adc!(abx!()),
            0x79 => adc!(aby!()),
            0x61 => adc!(zpxi!()),
            0x71 => adc!(zpiy!()),

            0xE9 => sbc!(imm!()),
            0xE5 => sbc!(zp!()),
            0xF5 => sbc!(zpx!()),
            0xED => sbc!(abs!()),
            0xFD => sbc!(abx!()),
            0xF9 => sbc!(aby!()),
            0xE1 => sbc!(zpxi!()),
            0xF1 => sbc!(zpiy!()),

            0xC9 => cmp!(a, imm!()),
            0xC5 => cmp!(a, zp!()),
            0xD5 => cmp!(a, zpx!()),
            0xCD => cmp!(a, abs!()),
            0xDD => cmp!(a, abx!()),
            0xD9 => cmp!(a, aby!()),
            0xC1 => cmp!(a, zpxi!()),
            0xD1 => cmp!(a, zpiy!()),

            0xE0 => cmp!(x, imm!()),
            0xE4 => cmp!(x, zp!()),
            0xEC => cmp!(x, abs!()),

            0xC0 => cmp!(y, imm!()),
            0xC4 => cmp!(y, zp!()),
            0xCC => cmp!(y, abs!()),

            0x29 => and!(imm!()),
            0x25 => and!(zp!()),
            0x35 => and!(zpx!()),
            0x2D => and!(abs!()),
            0x3D => and!(abx!()),
            0x39 => and!(aby!()),
            0x21 => and!(zpxi!()),
            0x31 => and!(zpiy!()),

            0x09 => ora!(imm!()),
            0x05 => ora!(zp!()),
            0x15 => ora!(zpx!()),
            0x0D => ora!(abs!()),
            0x1D => ora!(abx!()),
            0x19 => ora!(aby!()),
            0x01 => ora!(zpxi!()),
            0x11 => ora!(zpiy!()),

            0x49 => eor!(imm!()),
            0x45 => eor!(zp!()),
            0x55 => eor!(zpx!()),
            0x4D => eor!(abs!()),
            0x5D => eor!(abx!()),
            0x59 => eor!(aby!()),
            0x41 => eor!(zpxi!()),
            0x51 => eor!(zpiy!()),

            0x24 => bit!(zp!()),
            0x2C => bit!(abs!()),

            0xA9 => load!(a, imm!()),
            0xA5 => load!(a, zp!()),
            0xB5 => load!(a, zpx!()),
            0xAD => load!(a, abs!()),
            0xBD => load!(a, abx!()),
            0xB9 => load!(a, aby!()),
            0xA1 => load!(a, zpxi!()),
            0xB1 => load!(a, zpiy!()),

            0xA2 => load!(x, imm!()),
            0xA6 => load!(x, zp!()),
            0xB6 => load!(x, zpy!()),
            0xAE => load!(x, abs!()),
            0xBE => load!(x, aby!()),

            0xA0 => load!(y, imm!()),
            0xA4 => load!(y, zp!()),
            0xB4 => load!(y, zpx!()),
            0xAC => load!(y, abs!()),
            0xBC => load!(y, abx!()),

            0x85 => store!(a, zp!()),
            0x95 => store!(a, zpx!()),
            0x8D => store!(a, abs!()),
            0x9D => store!(a, abx!()),
            0x99 => store!(a, aby!()),
            0x81 => store!(a, zpxi!()),
            0x91 => store!(a, zpiy!()),

            0x86 => store!(x, zp!()),
            0x96 => store!(x, zpy!()),
            0x8E => store!(x, abs!()),

            0x84 => store!(y, zp!()),
            0x94 => store!(y, zpx!()),
            0x8C => store!(y, abs!()),

            0xAA => mov!(x, a),
            0xA8 => mov!(y, a),
            0x8A => mov!(a, x),
            0x98 => mov!(a, y),
            0xBA => mov!(x, s),
            0x9A => mov!(s, x, false),

            0x0A => modify!(asl, a),
            0x06 => read_modify_write!(asl, zp!()),
            0x16 => read_modify_write!(asl, zpx!()),
            0x0E => read_modify_write!(asl, abs!()),
            0x1E => read_modify_write!(asl, abx!()),

            0x4A => modify!(lsr, a),
            0x46 => read_modify_write!(lsr, zp!()),
            0x56 => read_modify_write!(lsr, zpx!()),
            0x4E => read_modify_write!(lsr, abs!()),
            0x5E => read_modify_write!(lsr, abx!()),

            0x2A => modify!(rol, a),
            0x26 => read_modify_write!(rol, zp!()),
            0x36 => read_modify_write!(rol, zpx!()),
            0x2E => read_modify_write!(rol, abs!()),
            0x3E => read_modify_write!(rol, abx!()),

            0x6A => modify!(ror, a),
            0x66 => read_modify_write!(ror, zp!()),
            0x76 => read_modify_write!(ror, zpx!()),
            0x6E => read_modify_write!(ror, abs!()),
            0x7E => read_modify_write!(ror, abx!()),

            0xE6 => read_modify_write!(inc, zp!()),
            0xF6 => read_modify_write!(inc, zpx!()),
            0xEE => read_modify_write!(inc, abs!()),
            0xFE => read_modify_write!(inc, abx!()),
            0xE8 => modify!(inc, x),
            0xC8 => modify!(inc, y),

            0xC6 => read_modify_write!(dec, zp!()),
            0xD6 => read_modify_write!(dec, zpx!()),
            0xCE => read_modify_write!(dec, abs!()),
            0xDE => read_modify_write!(dec, abx!()),
            0xCA => modify!(dec, x),
            0x88 => modify!(dec, y),

            0x90 => bra!(c, false),
            0xB0 => bra!(c, true),
            0xD0 => bra!(z, false),
            0xF0 => bra!(z, true),
            0x10 => bra!(n, false),
            0x30 => bra!(n, true),
            0x50 => bra!(v, false),
            0x70 => bra!(v, true),

            0x4C => self.reg.pc = abs!(),  // JMP abs
            0x6C => self.reg.pc = absi!(), // JMP (abs)

            // JSR
            0x20 => {
                self.push_u16(self.reg.pc.wrapping_add(1));
                self.reg.pc = abs!();
            }

            0x60 => self.reg.pc = self.pop_u16().wrapping_add(1), // RTS

            // RTI
            0x40 => {
                let f = self.pop_u8();
                self.reg.flag.set_u8(f);
                self.reg.pc = self.pop_u16()
            }

            0x38 => self.reg.flag.c = true, // SEC
            0xF8 => self.reg.flag.d = true, // SED
            0x78 => self.reg.flag.i = true, // SEI

            0x18 => self.reg.flag.c = false, // CLC
            0xD8 => self.reg.flag.d = false, // CLD
            0x58 => self.reg.flag.i = false, // CLI
            0xB8 => self.reg.flag.v = false, // CLV

            0x48 => self.push_u8(self.reg.a),              // PHA
            0x08 => self.push_u8(self.reg.flag.get_u8(3)), // PHP

            // PLA
            0x68 => {
                self.reg.a = self.pop_u8();
                self.reg.flag.set_nz(self.reg.a);
            }
            // PLP
            0x28 => {
                let p = self.pop_u8();
                self.reg.flag.set_u8(p);
            }

            // BRK
            0x00 => {
                self.reg.pc = self.reg.pc.wrapping_add(1);
                self.exec_interrupt(Interrupt::Irq, true);
            }

            0xEA => self.counter += 1, // NOP

            _ => {
                log::warn!("invalid opcode: ${opc:02X}");
            }
        }
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
        self.reg.pc = self.read_u16(vect);
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

    fn read_u16(&mut self, addr: u16) -> u16 {
        self.read_u8(addr) as u16 | (self.read_u8(addr + 1) as u16) << 8
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
            AddrMode::IND => format!(" = {}", read_u8(opr)),
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
                let addr = opr.wrapping_add(self.reg.x as u16);
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

macro_rules! instr_table {
    ($($mne:ident $addr_mode:ident,)*) => {{
        [$(
            (stringify!($mne), AddrMode::$addr_mode),
        )*]
    }}
}

#[rustfmt::skip]
const INSTR_TABLE: [(&str, AddrMode); 256] = instr_table! {
    BRK IMM, ORA INX, UNK UNK, UNK UNK, UNK UNK, ORA ZPG, ASL ZPG, UNK UNK,
    PHP IMP, ORA IMM, ASL ACC, UNK UNK, UNK UNK, ORA ABS, ASL ABS, UNK UNK,
    BPL REL, ORA INY, UNK UNK, UNK UNK, UNK UNK, ORA ZPX, ASL ZPX, UNK UNK,
    CLC IMP, ORA ABY, UNK UNK, UNK UNK, UNK UNK, ORA ABX, ASL ABX, UNK UNK,
    JSR ABS, AND INX, UNK UNK, UNK UNK, BIT ZPG, AND ZPG, ROL ZPG, UNK UNK,
    PLP IMP, AND IMM, ROL ACC, UNK UNK, BIT ABS, AND ABS, ROL ABS, UNK UNK,
    BMI REL, AND INY, UNK UNK, UNK UNK, UNK UNK, AND ZPX, ROL ZPX, UNK UNK,
    SEC IMP, AND ABY, UNK UNK, UNK UNK, UNK UNK, AND ABX, ROL ABX, UNK UNK,
    RTI IMP, EOR INX, UNK UNK, UNK UNK, UNK UNK, EOR ZPG, LSR ZPG, UNK UNK,
    PHA IMP, EOR IMM, LSR ACC, UNK UNK, JMP ABS, EOR ABS, LSR ABS, UNK UNK,
    BVC REL, EOR INY, UNK UNK, UNK UNK, UNK UNK, EOR ZPX, LSR ZPX, UNK UNK,
    CLI IMP, EOR ABY, UNK UNK, UNK UNK, UNK UNK, EOR ABX, LSR ABX, UNK UNK,
    RTS IMP, ADC INX, UNK UNK, UNK UNK, UNK UNK, ADC ZPG, ROR ZPG, UNK UNK,
    PLA IMP, ADC IMM, ROR ACC, UNK UNK, JMP IND, ADC ABS, ROR ABS, UNK UNK,
    BVS REL, ADC INY, UNK UNK, UNK UNK, UNK UNK, ADC ZPX, ROR ZPX, UNK UNK,
    SEI IMP, ADC ABY, UNK UNK, UNK UNK, UNK UNK, ADC ABX, ROR ABX, UNK UNK,
    UNK UNK, STA INX, UNK UNK, UNK UNK, STY ZPG, STA ZPG, STX ZPG, UNK UNK,
    DEY IMP, UNK UNK, TXA IMP, UNK UNK, STY ABS, STA ABS, STX ABS, UNK UNK,
    BCC REL, STA INY, UNK UNK, UNK UNK, STY ZPX, STA ZPX, STX ZPY, UNK UNK,
    TYA IMP, STA ABY, TXS IMP, UNK UNK, UNK UNK, STA ABX, UNK UNK, UNK UNK,
    LDY IMM, LDA INX, LDX IMM, UNK UNK, LDY ZPG, LDA ZPG, LDX ZPG, UNK UNK,
    TAY IMP, LDA IMM, TAX IMP, UNK UNK, LDY ABS, LDA ABS, LDX ABS, UNK UNK,
    BCS REL, LDA INY, UNK UNK, UNK UNK, LDY ZPX, LDA ZPX, LDX ZPY, UNK UNK,
    CLV IMP, LDA ABY, TSX IMP, UNK UNK, LDY ABX, LDA ABX, LDX ABY, UNK UNK,
    CPY IMM, CMP INX, UNK UNK, UNK UNK, CPY ZPG, CMP ZPG, DEC ZPG, UNK UNK,
    INY IMP, CMP IMM, DEX IMP, UNK UNK, CPY ABS, CMP ABS, DEC ABS, UNK UNK,
    BNE REL, CMP INY, UNK UNK, UNK UNK, UNK UNK, CMP ZPX, DEC ZPX, UNK UNK,
    CLD IMP, CMP ABY, UNK UNK, UNK UNK, UNK UNK, CMP ABX, DEC ABX, UNK UNK,
    CPX IMM, SBC INX, UNK UNK, UNK UNK, CPX ZPG, SBC ZPG, INC ZPG, UNK UNK,
    INX IMP, SBC IMM, NOP IMP, UNK UNK, CPX ABS, SBC ABS, INC ABS, UNK UNK,
    BEQ REL, SBC INY, UNK UNK, UNK UNK, UNK UNK, SBC ZPX, INC ZPX, UNK UNK,
    SED IMP, SBC ABY, UNK UNK, UNK UNK, UNK UNK, SBC ABX, INC ABX, UNK UNK,
};

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
