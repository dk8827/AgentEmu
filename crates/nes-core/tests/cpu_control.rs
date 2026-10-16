use nes_core::cpu::{B, Bus, Cpu, D, I, U};

struct Memory {
    bytes: Vec<u8>,
    trace: Vec<(u16, u8, bool)>,
}
impl Memory {
    fn new() -> Self {
        Self {
            bytes: vec![0; 65536],
            trace: Vec::new(),
        }
    }
}
impl Bus for Memory {
    fn read(&mut self, address: u16, _: u64) -> u8 {
        let value = self.bytes[address as usize];
        self.trace.push((address, value, false));
        value
    }
    fn write(&mut self, address: u16, value: u8, _: u64) {
        self.bytes[address as usize] = value;
        self.trace.push((address, value, true));
    }
}

#[test]
fn reset_reads_stack_without_writing_and_preserves_registers() {
    let mut bus = Memory::new();
    bus.bytes[0xfffc] = 0x34;
    bus.bytes[0xfffd] = 0x12;
    let mut cpu = Cpu {
        pc: 0x8000,
        sp: 0,
        a: 0x7f,
        x: 2,
        y: 3,
        p: D | U,
        cycles: 0,
    };
    cpu.reset(&mut bus);
    assert_eq!(cpu.pc, 0x1234);
    assert_eq!(cpu.sp, 0xfd);
    assert_eq!(cpu.cycles, 7);
    assert_eq!((cpu.a, cpu.x, cpu.y, cpu.p), (0x7f, 2, 3, D | U | I));
    assert_eq!(
        bus.trace.iter().map(|t| t.0).collect::<Vec<_>>(),
        [0x8000, 0x8000, 0x100, 0x1ff, 0x1fe, 0xfffc, 0xfffd]
    );
    assert!(bus.trace.iter().all(|t| !t.2));
}

#[test]
fn interrupt_pushes_next_pc_and_status_without_break_then_rti_restores() {
    for nmi in [false, true] {
        let mut bus = Memory::new();
        let vector = if nmi { 0xfffa } else { 0xfffe };
        bus.bytes[vector] = 0;
        bus.bytes[vector + 1] = 0x90;
        bus.bytes[0x9000] = 0x40;
        let mut cpu = Cpu {
            pc: 0x8123,
            sp: 0xfd,
            p: U | D,
            ..Default::default()
        };
        cpu.interrupt(&mut bus, nmi);
        assert_eq!((cpu.pc, cpu.sp, cpu.cycles), (0x9000, 0xfa, 7));
        assert_eq!(&bus.bytes[0x1fb..=0x1fd], &[U | D, 0x23, 0x81]);
        assert_eq!(bus.bytes[0x1fb] & B, 0);
        assert_eq!(cpu.step(&mut bus).unwrap(), 6);
        assert_eq!((cpu.pc, cpu.sp, cpu.p), (0x8123, 0xfd, U | D));
    }
}

#[test]
fn decimal_flag_does_not_enable_bcd_and_unknown_opcodes_are_explicit() {
    let mut bus = Memory::new();
    bus.bytes[..3].copy_from_slice(&[0x69, 0x55, 0x02]);
    let mut cpu = Cpu {
        a: 0x45,
        p: U | D,
        ..Default::default()
    };
    cpu.step(&mut bus).unwrap();
    assert_eq!(cpu.a, 0x9a);
    let error = cpu.step(&mut bus).unwrap_err();
    assert_eq!((error.address, error.opcode), (2, 2));
}
