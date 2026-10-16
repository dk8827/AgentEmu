//! Ricoh 2A03 CPU. Every instruction performs its actual ordered bus accesses.
//! Decimal mode is stored in P but does not alter ADC/SBC on this CPU.

use std::{error::Error, fmt};

pub const C: u8 = 0x01;
pub const Z: u8 = 0x02;
pub const I: u8 = 0x04;
pub const D: u8 = 0x08;
pub const B: u8 = 0x10;
pub const U: u8 = 0x20;
pub const V: u8 = 0x40;
pub const N: u8 = 0x80;

/// A read/write is one CPU cycle. The machine bus can clock other hardware here.
/// The timestamp identifies the start of that cycle, including dummy accesses.
pub trait Bus {
    fn read(&mut self, address: u16, cycle: u64) -> u8;
    fn write(&mut self, address: u16, value: u8, cycle: u64);
    /// Read-cycle DMA stalls accumulated by the bus.
    fn take_stall_cycles(&mut self) -> u64 {
        0
    }
    fn poll_interrupts(&mut self, _disabled: bool) {}
    /// An NMI edge before vector fetch can hijack IRQ or BRK entry.
    fn take_nmi(&mut self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Cpu {
    pub pc: u16,
    pub sp: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub p: u8,
    pub cycles: u64,
}
impl Default for Cpu {
    fn default() -> Self {
        Self {
            pc: 0,
            sp: 0,
            a: 0,
            x: 0,
            y: 0,
            p: I | U,
            cycles: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UnsupportedOpcode {
    pub address: u16,
    pub opcode: u8,
}
impl fmt::Display for UnsupportedOpcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsupported opcode {:02X} at {:04X}",
            self.opcode, self.address
        )
    }
}
impl Error for UnsupportedOpcode {}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Imm,
    Zp,
    ZpX,
    ZpY,
    Abs,
    AbsX,
    AbsY,
    IndX,
    IndY,
    Acc,
    Imp,
    Rel,
    Ind,
}
#[derive(Clone, Copy, Debug)]
enum Op {
    Adc,
    And,
    Asl,
    Bcc,
    Bcs,
    Beq,
    Bit,
    Bmi,
    Bne,
    Bpl,
    Brk,
    Bvc,
    Bvs,
    Clc,
    Cld,
    Cli,
    Clv,
    Cmp,
    Cpx,
    Cpy,
    Dec,
    Dex,
    Dey,
    Eor,
    Inc,
    Inx,
    Iny,
    Jmp,
    Jsr,
    Lda,
    Ldx,
    Ldy,
    Lsr,
    Nop,
    Ora,
    Pha,
    Php,
    Pla,
    Plp,
    Rol,
    Ror,
    Rti,
    Rts,
    Sbc,
    Sec,
    Sed,
    Sei,
    Sta,
    Stx,
    Sty,
    Tax,
    Tay,
    Tsx,
    Txa,
    Txs,
    Tya,
}

include!("opcodes.rs");

pub fn supported_opcodes() -> impl Iterator<Item = u8> {
    (0..=255).filter(|opcode| decode(*opcode).is_some())
}

impl Cpu {
    fn read(&mut self, bus: &mut impl Bus, addr: u16) -> u8 {
        let result = bus.read(addr, self.cycles);
        self.cycles = self.cycles.wrapping_add(1 + bus.take_stall_cycles());
        result
    }
    fn write(&mut self, bus: &mut impl Bus, addr: u16, value: u8) {
        bus.write(addr, value, self.cycles);
        self.cycles = self.cycles.wrapping_add(1);
    }
    fn fetch(&mut self, bus: &mut impl Bus) -> u8 {
        let value = self.read(bus, self.pc);
        self.pc = self.pc.wrapping_add(1);
        value
    }
    fn word(&mut self, bus: &mut impl Bus) -> u16 {
        let low = self.fetch(bus);
        u16::from_le_bytes([low, self.fetch(bus)])
    }
    fn push(&mut self, bus: &mut impl Bus, value: u8) {
        self.write(bus, 0x100 | self.sp as u16, value);
        self.sp = self.sp.wrapping_sub(1);
    }
    fn pull(&mut self, bus: &mut impl Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        self.read(bus, 0x100 | self.sp as u16)
    }
    fn flag(&mut self, mask: u8, set: bool) {
        self.p = (self.p & !mask) | if set { mask } else { 0 };
    }
    fn nz(&mut self, value: u8) {
        self.flag(N, value & N != 0);
        self.flag(Z, value == 0);
    }

    /// Hardware reset: three stack reads decrement SP; RAM and registers persist.
    pub fn reset(&mut self, bus: &mut impl Bus) {
        self.read(bus, self.pc);
        self.read(bus, self.pc);
        for _ in 0..3 {
            self.read(bus, 0x100 | self.sp as u16);
            self.sp = self.sp.wrapping_sub(1);
        }
        self.p = (self.p | I | U) & !B;
        let low = self.read(bus, 0xfffc);
        self.pc = u16::from_le_bytes([low, self.read(bus, 0xfffd)]);
    }

    /// Service an already sampled interrupt at an instruction boundary.
    /// IRQ polling delay and NMI edge arbitration belong to the machine scheduler.
    pub fn interrupt(&mut self, bus: &mut impl Bus, nmi: bool) {
        self.read(bus, self.pc);
        self.read(bus, self.pc);
        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, self.pc as u8);
        self.push(bus, (self.p | U) & !B);
        self.p |= I;
        let vector = if nmi || bus.take_nmi() {
            0xfffa
        } else {
            0xfffe
        };
        let low = self.read(bus, vector);
        self.pc = u16::from_le_bytes([low, self.read(bus, vector + 1)]);
    }

    fn address(&mut self, bus: &mut impl Bus, mode: Mode, write: bool) -> u16 {
        match mode {
            Mode::Zp => self.fetch(bus) as u16,
            Mode::ZpX | Mode::ZpY => {
                let base = self.fetch(bus);
                self.read(bus, base as u16);
                base.wrapping_add(if matches!(mode, Mode::ZpX) {
                    self.x
                } else {
                    self.y
                }) as u16
            }
            Mode::Abs => self.word(bus),
            Mode::AbsX | Mode::AbsY => {
                let base = self.word(bus);
                let addr = base.wrapping_add(if matches!(mode, Mode::AbsX) {
                    self.x
                } else {
                    self.y
                } as u16);
                if write || base & 0xff00 != addr & 0xff00 {
                    self.read(bus, (base & 0xff00) | (addr & 0xff));
                }
                addr
            }
            Mode::IndX => {
                let base = self.fetch(bus);
                self.read(bus, base as u16);
                let ptr = base.wrapping_add(self.x);
                let low = self.read(bus, ptr as u16);
                u16::from_le_bytes([low, self.read(bus, ptr.wrapping_add(1) as u16)])
            }
            Mode::IndY => {
                let ptr = self.fetch(bus);
                let low = self.read(bus, ptr as u16);
                let base = u16::from_le_bytes([low, self.read(bus, ptr.wrapping_add(1) as u16)]);
                let addr = base.wrapping_add(self.y as u16);
                if write || base & 0xff00 != addr & 0xff00 {
                    self.read(bus, (base & 0xff00) | (addr & 0xff));
                }
                addr
            }
            _ => unreachable!("address mode {mode:?}"),
        }
    }
    fn operand(&mut self, bus: &mut impl Bus, mode: Mode) -> u8 {
        if matches!(mode, Mode::Imm) {
            self.fetch(bus)
        } else {
            let addr = self.address(bus, mode, false);
            self.read(bus, addr)
        }
    }
    fn adc(&mut self, value: u8) {
        let sum = self.a as u16 + value as u16 + (self.p & C) as u16;
        let result = sum as u8;
        self.flag(V, (!(self.a ^ value) & (self.a ^ result) & 0x80) != 0);
        self.flag(C, sum > 0xff);
        self.a = result;
        self.nz(result);
    }
    fn compare(&mut self, reg: u8, value: u8) {
        self.flag(C, reg >= value);
        self.nz(reg.wrapping_sub(value));
    }

    pub fn step(&mut self, bus: &mut impl Bus) -> Result<u64, UnsupportedOpcode> {
        let start = self.cycles;
        let old_i = self.p & I != 0;
        let address = self.pc;
        let opcode = self.fetch(bus);
        let (op, mode) = decode(opcode).ok_or(UnsupportedOpcode { address, opcode })?;
        use Op::*;
        match op {
            Adc | And | Bit | Cmp | Cpx | Cpy | Eor | Lda | Ldx | Ldy | Ora | Sbc => {
                let value = self.operand(bus, mode);
                match op {
                    Adc => self.adc(value),
                    Sbc => self.adc(!value),
                    And => {
                        self.a &= value;
                        self.nz(self.a);
                    }
                    Eor => {
                        self.a ^= value;
                        self.nz(self.a);
                    }
                    Ora => {
                        self.a |= value;
                        self.nz(self.a);
                    }
                    Bit => {
                        self.flag(Z, self.a & value == 0);
                        self.p = (self.p & !(N | V)) | (value & (N | V));
                    }
                    Cmp => self.compare(self.a, value),
                    Cpx => self.compare(self.x, value),
                    Cpy => self.compare(self.y, value),
                    Lda => {
                        self.a = value;
                        self.nz(value);
                    }
                    Ldx => {
                        self.x = value;
                        self.nz(value);
                    }
                    Ldy => {
                        self.y = value;
                        self.nz(value);
                    }
                    _ => unreachable!(),
                }
            }
            Sta | Stx | Sty => {
                let addr = self.address(bus, mode, true);
                self.write(
                    bus,
                    addr,
                    match op {
                        Sta => self.a,
                        Stx => self.x,
                        _ => self.y,
                    },
                );
            }
            Asl | Lsr | Rol | Ror | Inc | Dec => {
                let accumulator = matches!(mode, Mode::Acc);
                let (addr, old) = if accumulator {
                    self.read(bus, self.pc);
                    (0, self.a)
                } else {
                    let addr = self.address(bus, mode, true);
                    let value = self.read(bus, addr);
                    self.write(bus, addr, value);
                    (addr, value)
                };
                let carry = self.p & C;
                let value = match op {
                    Asl => {
                        self.flag(C, old & 0x80 != 0);
                        old << 1
                    }
                    Lsr => {
                        self.flag(C, old & 1 != 0);
                        old >> 1
                    }
                    Rol => {
                        self.flag(C, old & 0x80 != 0);
                        (old << 1) | carry
                    }
                    Ror => {
                        self.flag(C, old & 1 != 0);
                        (old >> 1) | (carry << 7)
                    }
                    Inc => old.wrapping_add(1),
                    Dec => old.wrapping_sub(1),
                    _ => unreachable!(),
                };
                self.nz(value);
                if accumulator {
                    self.a = value;
                } else {
                    self.write(bus, addr, value);
                }
            }
            Bcc | Bcs | Beq | Bmi | Bne | Bpl | Bvc | Bvs => {
                let offset = self.fetch(bus) as i8;
                bus.poll_interrupts(self.p & I != 0);
                let take = match op {
                    Bcc => self.p & C == 0,
                    Bcs => self.p & C != 0,
                    Beq => self.p & Z != 0,
                    Bne => self.p & Z == 0,
                    Bmi => self.p & N != 0,
                    Bpl => self.p & N == 0,
                    Bvc => self.p & V == 0,
                    Bvs => self.p & V != 0,
                    _ => unreachable!(),
                };
                if take {
                    self.read(bus, self.pc);
                    let target = self.pc.wrapping_add_signed(offset as i16);
                    if target & 0xff00 != self.pc & 0xff00 {
                        self.read(bus, (self.pc & 0xff00) | (target & 0xff));
                        bus.poll_interrupts(self.p & I != 0);
                    }
                    self.pc = target;
                }
            }
            Jmp => {
                let ptr = self.word(bus);
                self.pc = if matches!(mode, Mode::Ind) {
                    let low = self.read(bus, ptr);
                    let high = self.read(bus, (ptr & 0xff00) | (ptr.wrapping_add(1) & 0xff));
                    u16::from_le_bytes([low, high])
                } else {
                    ptr
                };
            }
            Jsr => {
                let low = self.fetch(bus);
                self.read(bus, 0x100 | self.sp as u16);
                self.push(bus, (self.pc >> 8) as u8);
                self.push(bus, self.pc as u8);
                self.pc = u16::from_le_bytes([low, self.fetch(bus)]);
            }
            Rts | Rti => {
                self.read(bus, self.pc);
                self.read(bus, 0x100 | self.sp as u16);
                if matches!(op, Rti) {
                    self.p = (self.pull(bus) | U) & !B;
                }
                let low = self.pull(bus);
                self.pc = u16::from_le_bytes([low, self.pull(bus)]);
                if matches!(op, Rts) {
                    self.read(bus, self.pc);
                    self.pc = self.pc.wrapping_add(1);
                }
            }
            Brk => {
                self.fetch(bus);
                self.push(bus, (self.pc >> 8) as u8);
                self.push(bus, self.pc as u8);
                self.push(bus, self.p | B | U);
                self.p |= I;
                let vector = if bus.take_nmi() { 0xfffa } else { 0xfffe };
                let low = self.read(bus, vector);
                self.pc = u16::from_le_bytes([low, self.read(bus, vector + 1)]);
            }
            Pha | Php => {
                self.read(bus, self.pc);
                self.push(
                    bus,
                    if matches!(op, Pha) {
                        self.a
                    } else {
                        self.p | B | U
                    },
                );
            }
            Pla | Plp => {
                self.read(bus, self.pc);
                self.read(bus, 0x100 | self.sp as u16);
                let value = self.pull(bus);
                if matches!(op, Pla) {
                    self.a = value;
                    self.nz(value);
                } else {
                    self.p = (value | U) & !B;
                }
            }
            _ => {
                self.read(bus, self.pc);
                match op {
                    Clc => self.p &= !C,
                    Cld => self.p &= !D,
                    Cli => self.p &= !I,
                    Clv => self.p &= !V,
                    Sec => self.p |= C,
                    Sed => self.p |= D,
                    Sei => self.p |= I,
                    Dex => {
                        self.x = self.x.wrapping_sub(1);
                        self.nz(self.x);
                    }
                    Dey => {
                        self.y = self.y.wrapping_sub(1);
                        self.nz(self.y);
                    }
                    Inx => {
                        self.x = self.x.wrapping_add(1);
                        self.nz(self.x);
                    }
                    Iny => {
                        self.y = self.y.wrapping_add(1);
                        self.nz(self.y);
                    }
                    Tax => {
                        self.x = self.a;
                        self.nz(self.x);
                    }
                    Tay => {
                        self.y = self.a;
                        self.nz(self.y);
                    }
                    Tsx => {
                        self.x = self.sp;
                        self.nz(self.x);
                    }
                    Txa => {
                        self.a = self.x;
                        self.nz(self.a);
                    }
                    Txs => self.sp = self.x,
                    Tya => {
                        self.a = self.y;
                        self.nz(self.a);
                    }
                    Nop => (),
                    _ => unreachable!(),
                }
            }
        }
        if !matches!(op, Brk | Bcc | Bcs | Beq | Bmi | Bne | Bpl | Bvc | Bvs) {
            bus.poll_interrupts(if matches!(op, Cli | Sei | Plp) {
                old_i
            } else {
                self.p & I != 0
            });
        }
        Ok(self.cycles.wrapping_sub(start))
    }
}
