use nes_core::{
    cartridge::{Cartridge, Mirroring},
    controller::Controller,
    cpu::{Bus, Cpu},
    diagnostic::controller_rom,
    machine::Machine,
};

fn image(mapper: u8, prg: u8, chr: u8) -> Vec<u8> {
    let mut bytes = vec![0; 16 + prg as usize * 16384 + chr as usize * 8192];
    bytes[..4].copy_from_slice(b"NES\x1a");
    bytes[4] = prg;
    bytes[5] = chr;
    bytes[6] = mapper << 4;
    for bank in 0..prg as usize {
        bytes[16 + bank * 16384..16 + (bank + 1) * 16384].fill(bank as u8);
    }
    for bank in 0..chr as usize * 2 {
        let offset = 16 + prg as usize * 16384 + bank * 4096;
        bytes[offset..offset + 4096].fill(bank as u8);
    }
    bytes
}
fn serial(cart: &mut Cartridge, address: u16, value: u8, cycle: &mut u64) {
    for bit in 0..5 {
        cart.cpu_write(address, (value >> bit) & 1, *cycle);
        *cycle += 2;
    }
}

#[test]
fn joypad_latches_both_edges_reads_live_a_when_high_and_returns_one_after_eight_bits() {
    let mut pad = Controller::default();
    pad.set_buttons(0b1010_0101);
    pad.strobe(true);
    assert_eq!(pad.read(), 1);
    assert_eq!(pad.read(), 1);
    pad.set_buttons(0b1010_0100);
    assert_eq!(pad.read(), 0);
    pad.strobe(false);
    pad.set_buttons(0xff);
    for expected in [0, 0, 1, 0, 0, 1, 0, 1, 1, 1] {
        assert_eq!(pad.peek(), expected);
        assert_eq!(pad.read(), expected);
    }
}

#[test]
fn nrom_mirrors_prg_and_has_writable_chr_ram_but_readonly_chr_rom() {
    let mut bytes = image(0, 1, 0);
    bytes[16 + 0x1234] = 0x7f;
    let mut cart = Cartridge::from_ines(&bytes).unwrap();
    assert_eq!(cart.cpu_read(0x9234), Some(0x7f));
    assert_eq!(cart.cpu_read(0xd234), Some(0x7f));
    cart.cpu_write(0x9234, 0, 1);
    assert_eq!(cart.cpu_read(0x9234), Some(0x7f));
    cart.chr_write(0x1fff, 0x42);
    assert_eq!(cart.chr_read(0x1fff), 0x42);
    let mut cart = Cartridge::from_ines(&image(0, 2, 1)).unwrap();
    cart.chr_write(0x1000, 0x42);
    assert_eq!(cart.chr_read(0x1000), 1);
}

#[test]
fn uxrom_switches_lower_bank_keeps_upper_fixed_and_models_bus_conflicts() {
    let mut bytes = image(2, 4, 0);
    bytes[16 + 0x100] = 0xff;
    let mut cart = Cartridge::from_ines(&bytes).unwrap();
    cart.cpu_write(0x8100, 2, 0);
    assert_eq!(cart.cpu_read(0x8000), Some(2));
    assert_eq!(cart.cpu_read(0xc000), Some(3));
    cart.cpu_write(0x8000, 1, 2); // selected ROM drives 2: 1 & 2 = 0
    assert_eq!(cart.cpu_read(0x8000), Some(0));
}

#[test]
fn mmc1_prg_chr_ram_enable_mirroring_and_shift_reset() {
    let mut cart = Cartridge::from_ines(&image(1, 4, 4)).unwrap();
    let mut cycle = 0;
    assert_eq!(cart.cpu_read(0xc000), Some(3));
    serial(&mut cart, 0xe000, 2, &mut cycle);
    assert_eq!(cart.cpu_read(0x8000), Some(2));
    serial(&mut cart, 0x8000, 0x1e, &mut cycle);
    serial(&mut cart, 0xa000, 3, &mut cycle);
    serial(&mut cart, 0xc000, 6, &mut cycle);
    assert_eq!(cart.mirroring(), Mirroring::Vertical);
    assert_eq!(cart.chr_read(0), 3);
    assert_eq!(cart.chr_read(0x1000), 6);
    cart.cpu_write(0x6000, 0x77, cycle);
    cycle += 2;
    serial(&mut cart, 0xe000, 0x10, &mut cycle);
    assert_eq!(cart.cpu_read(0x6000), None);
    cart.cpu_write(0x6000, 0x99, cycle);
    cycle += 2;
    serial(&mut cart, 0xe000, 0, &mut cycle);
    assert_eq!(cart.cpu_read(0x6000), Some(0x77));
    serial(&mut cart, 0x8000, 0, &mut cycle);
    serial(&mut cart, 0xe000, 2, &mut cycle);
    assert_eq!(cart.cpu_read(0x8000), Some(2));
    assert_eq!(cart.cpu_read(0xc000), Some(3));
    cart.cpu_write(0x8000, 0x80, cycle);
    cycle += 2;
    serial(&mut cart, 0xe000, 1, &mut cycle);
    assert_eq!(cart.cpu_read(0x8000), Some(1));
    assert_eq!(cart.cpu_read(0xc000), Some(3));
}

#[test]
fn mmc1_ignores_second_write_of_real_cpu_read_modify_write() {
    struct CartridgeBus(Cartridge);
    impl Bus for CartridgeBus {
        fn read(&mut self, address: u16, _: u64) -> u8 {
            self.0.cpu_read(address).unwrap_or(0)
        }
        fn write(&mut self, address: u16, value: u8, cycle: u64) {
            self.0.cpu_write(address, value, cycle);
        }
    }
    let mut bytes = image(1, 4, 0);
    bytes[16..19].copy_from_slice(&[0xee, 0x00, 0xe0]); // INC $E000 writes 3 then 4.
    let mut bus = CartridgeBus(Cartridge::from_ines(&bytes).unwrap());
    let mut cpu = Cpu {
        pc: 0x8000,
        ..Default::default()
    };
    assert_eq!(cpu.step(&mut bus).unwrap(), 6);
    // First write contributed bit 1. Complete with four zero bits => bank 1.
    for cycle in [10, 12, 14, 16] {
        bus.0.cpu_write(0xe000, 0, cycle);
    }
    assert_eq!(bus.0.cpu_read(0x8000), Some(1));
}

#[test]
fn malformed_and_unsupported_images_are_errors_not_panics() {
    for bytes in [vec![], vec![0; 16], image(6, 1, 1), image(0, 0, 0)] {
        assert!(Cartridge::from_ines(&bytes).is_err());
    }
    let mut bytes = image(0, 1, 0);
    bytes.pop();
    assert!(Cartridge::from_ines(&bytes).is_err());
    let mut bytes = image(0, 1, 0);
    bytes[7] = 8;
    assert!(Cartridge::from_ines(&bytes).is_err());
    let mut bytes = image(0, 1, 0);
    bytes[9] = 1;
    assert!(Cartridge::from_ines(&bytes).unwrap().is_pal());
}

fn poll(machine: &mut Machine) {
    for _ in 0..1000 {
        machine.step().unwrap();
        if machine.cpu.pc == 0x8005 && machine.peek(0x20) == Some(0xa5) {
            return;
        }
    }
    panic!("diagnostic did not finish a controller poll");
}

#[test]
fn original_rom_reads_two_independent_ports_and_ram_mirrors() {
    let mut machine = Machine::new(Cartridge::from_ines(&controller_rom()).unwrap());
    machine.set_controllers([0xa5, 0x5a]);
    poll(&mut machine);
    for bit in 0..8 {
        assert_eq!(machine.peek(bit), Some((0xa5 >> bit) & 1));
        assert_eq!(machine.peek(0x10 + bit), Some((0x5a >> bit) & 1));
    }
    for mirror in [0x20, 0x820, 0x1020, 0x1820] {
        assert_eq!(machine.peek(mirror), Some(0xa5));
    }
    assert_eq!(machine.peek(0x21), Some(1));
}

#[test]
fn save_restore_preserves_mid_poll_serial_state_and_replays_exactly() {
    let rom = controller_rom();
    let mut machine = Machine::new(Cartridge::from_ines(&rom).unwrap());
    machine.set_controllers([0xa5, 0x5a]);
    machine.run_instructions(19).unwrap();
    let state = machine.save_state();
    for buttons in [[0, 255], [0x23, 0x12], [0x55, 0xaa]] {
        machine.set_controllers(buttons);
        machine.run_instructions(199).unwrap();
    }
    let expected = machine.save_state();
    machine.load_state(&state).unwrap();
    for buttons in [[0, 255], [0x23, 0x12], [0x55, 0xaa]] {
        machine.set_controllers(buttons);
        machine.run_instructions(199).unwrap();
    }
    assert_eq!(machine.save_state(), expected);
    let before = machine.save_state();
    let mut bad: serde_json::Value = serde_json::from_slice(&state).unwrap();
    bad["ram"] = serde_json::json!([]);
    assert!(
        machine
            .load_state(&serde_json::to_vec(&bad).unwrap())
            .is_err()
    );
    assert_eq!(machine.save_state(), before);
    let mut other = rom;
    other[16 + 100] = 1;
    let mut other_machine = Machine::new(Cartridge::from_ines(&other).unwrap());
    assert!(other_machine.load_state(&state).is_err());
}

#[test]
fn native_diagnostic_matches_shared_wasm_fixture() {
    let mut machine = Machine::new(Cartridge::from_ines(&controller_rom()).unwrap());
    machine.set_controllers([0xa5, 0x5a]);
    machine.run_instructions(1000).unwrap();
    let actual: serde_json::Value = serde_json::from_str(&machine.diagnostic_report()).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/diagnostic/native.json")).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn fatal_opcode_blocks_execution_until_valid_restore_or_reset() {
    let mut bytes = controller_rom();
    bytes[16] = 0x02;
    bytes[17] = 0xea;
    let mut m = Machine::new(Cartridge::from_ines(&bytes).unwrap());
    let before = m.save_state();
    let error = m.step().unwrap_err().to_string();
    let faulted = m.save_state();
    for _ in 0..3 {
        assert_eq!(m.run_frame().unwrap_err().to_string(), error);
        assert_eq!(m.save_state(), faulted);
    }
    assert!(m.load_state(b"invalid").is_err());
    assert!(m.load_state(&faulted).is_err());
    assert!(m.fault().is_some());
    m.load_state(&before).unwrap();
    assert!(m.fault().is_none());
    assert_eq!(m.step().unwrap_err().to_string(), error);
    let reset = Machine::new(Cartridge::from_ines(&bytes).unwrap());
    assert!(reset.fault().is_none());
}
