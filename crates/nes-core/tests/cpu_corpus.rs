use nes_core::cpu::{Bus, Cpu, supported_opcodes};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct State {
    pc: u16,
    s: u8,
    a: u8,
    x: u8,
    y: u8,
    p: u8,
    ram: Vec<(u16, u8)>,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    initial: State,
    #[serde(rename = "final")]
    final_state: State,
    cycles: Vec<(u16, u8, String)>,
}
struct Memory {
    ram: Vec<u8>,
    accesses: Vec<(u16, u8, String)>,
}
impl Bus for Memory {
    fn read(&mut self, address: u16, cycle: u64) -> u8 {
        assert_eq!(cycle as usize, self.accesses.len());
        let value = self.ram[address as usize];
        self.accesses.push((address, value, "read".into()));
        value
    }
    fn write(&mut self, address: u16, value: u8, cycle: u64) {
        assert_eq!(cycle as usize, self.accesses.len());
        self.ram[address as usize] = value;
        self.accesses.push((address, value, "write".into()));
    }
}

fn run(directory: &Path) -> usize {
    let mut count = 0;
    for opcode in supported_opcodes() {
        let path = directory.join(format!("{opcode:02x}.json"));
        let cases: Vec<Case> = serde_json::from_slice(
            &std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
        )
        .unwrap();
        for case in cases {
            let s = case.initial;
            let mut cpu = Cpu {
                pc: s.pc,
                sp: s.s,
                a: s.a,
                x: s.x,
                y: s.y,
                p: s.p,
                cycles: 0,
            };
            let mut memory = Memory {
                ram: vec![0; 65536],
                accesses: Vec::new(),
            };
            for (addr, value) in s.ram {
                memory.ram[addr as usize] = value;
            }
            let elapsed = cpu.step(&mut memory).unwrap();
            let f = case.final_state;
            assert_eq!(
                (cpu.pc, cpu.sp, cpu.a, cpu.x, cpu.y, cpu.p),
                (f.pc, f.s, f.a, f.x, f.y, f.p),
                "opcode {opcode:02x}: {} registers",
                case.name
            );
            assert_eq!(
                elapsed as usize,
                case.cycles.len(),
                "opcode {opcode:02x}: {} count",
                case.name
            );
            assert_eq!(
                memory.accesses, case.cycles,
                "opcode {opcode:02x}: {} bus",
                case.name
            );
            for (addr, value) in f.ram {
                assert_eq!(
                    memory.ram[addr as usize], value,
                    "opcode {opcode:02x}: {} memory {addr:04x}",
                    case.name
                );
            }
            count += 1;
        }
    }
    count
}

#[test]
fn official_opcodes_match_independent_bus_traces() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cpu");
    assert_eq!(supported_opcodes().count(), 151);
    assert_eq!(run(&root), 151 * 16);
}

#[test]
#[ignore = "download full corpus with python3 scripts/fetch-cpu-tests.py first"]
fn full_official_opcode_corpus() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cache/nes6502");
    let count = run(&root);
    assert_eq!(count, 1_510_000);
    println!("Passed {count} instruction cases including every bus cycle");
}
