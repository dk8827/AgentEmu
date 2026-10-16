use nes_core::{
    cartridge::Cartridge,
    diagnostic::{controller_rom, ppu_rom},
    machine::Machine,
};

fn machine_with(code: &[u8]) -> Machine {
    let mut rom = controller_rom();
    rom[16..16 + 0x3ffa].fill(0xea);
    rom[16..16 + code.len()].copy_from_slice(code);
    Machine::new(Cartridge::from_ines(&rom).unwrap())
}
fn state(m: &Machine) -> serde_json::Value {
    serde_json::from_slice(&m.save_state()).unwrap()
}

#[test]
fn oam_dma_copies_cpu_ram_with_wrapping_and_clocks_513_or_514_stall_cycles() {
    for odd in [true, false] {
        // Reset consumes 7 cycles. LDA/STA have even lengths; BIT flips parity.
        let mut code = vec![0xa9, 0x77, 0x8d, 0, 2, 0xa9, 0xf0, 0x8d, 3, 0x20];
        if !odd {
            code.extend([0x24, 0]);
        }
        code.extend([0xa9, 2, 0x8d, 0x14, 0x40]);
        let mut m = machine_with(&code);
        m.run_instructions(if odd { 5 } else { 6 }).unwrap();
        let before = m.cpu.cycles;
        assert_eq!(before & 1 != 0, odd);
        assert_eq!(m.step().unwrap(), 4 + if odd { 514 } else { 513 });
        assert_eq!(m.ppu().total_dots(), m.cpu.cycles * 3);
        assert_eq!(state(&m)["ppu"]["oam"][0xf0], 0x77);
        assert_eq!(state(&m)["ppu"]["oam_addr"], 0xf0);
        assert_eq!(m.cpu.pc as usize, 0x8000 + code.len());
    }
}

#[test]
fn rmw_on_dma_register_finishes_both_writes_before_one_transfer() {
    // ASL $4014 reads open bus $40, then writes $40 and $80. Only page $80
    // transfers; starting page $40 prematurely would hit unimplemented APU I/O.
    let mut m = machine_with(&[0x0e, 0x14, 0x40]);
    assert_eq!(m.step().unwrap(), 6 + 514);
    let s = state(&m);
    assert_eq!(s["ppu"]["oam"][0], 0x0e);
    assert_eq!(s["ppu"]["oam"][1], 0x14);
    assert_eq!(s["ppu"]["oam"][2], 0x40);
    assert_eq!(m.ppu().total_dots(), m.cpu.cycles * 3);
}

#[test]
fn original_ppu_rom_checks_memory_dma_and_repeated_nmi_with_stack_restoration() {
    let mut m = Machine::new(Cartridge::from_ines(&ppu_rom()).unwrap());
    m.run_instructions(50_000).unwrap();
    assert_eq!(
        (0..7).map(|a| m.peek(a).unwrap()).collect::<Vec<_>>(),
        [0xab, 0xcd, 0x42, 0x2a, 0, 2, 0xff]
    );
    assert_eq!(m.peek(0x20), Some(0xa5));
    assert!(
        m.peek(0x10).unwrap() >= 2,
        "NMI handler must run on multiple frames"
    );
    assert_eq!(m.cpu.sp, 0xff);
    assert_eq!(m.ppu().total_dots(), m.cpu.cycles * 3);
    assert_eq!(m.peek_ppu(0x100), 0xab);
    assert_eq!(m.peek_ppu(0x3f10), 0x2a);
    let actual: serde_json::Value = serde_json::from_str(&m.diagnostic_report()).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/diagnostic/ppu-native.json")).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn snapshots_replay_partial_address_writes_buffers_and_pending_nmi() {
    let mut m = Machine::new(Cartridge::from_ines(&ppu_rom()).unwrap());
    let mut saw_address = false;
    let mut saw_buffer = false;
    let mut saw_nmi = false;
    for step in 0..50_000 {
        m.step().unwrap();
        // Avoid serializing every wait-loop step; setup begins after ~16k steps.
        if step < 16_000 || m.cpu.pc < 0x8012 || (saw_address && saw_buffer && !m.ppu().nmi_line())
        {
            continue;
        }
        let s = state(&m);
        let checkpoint = (!saw_address && s["ppu"]["second_write"] == true)
            || (!saw_buffer && s["ppu"]["read_buffer"] == 0xab)
            || (!saw_nmi && s["nmi_pending"] == true);
        if checkpoint {
            saw_address |= s["ppu"]["second_write"] == true;
            saw_buffer |= s["ppu"]["read_buffer"] == 0xab;
            saw_nmi |= s["nmi_pending"] == true;
            let saved = m.save_state();
            m.run_instructions(100).unwrap();
            let expected = m.save_state();
            m.load_state(&saved).unwrap();
            m.run_instructions(100).unwrap();
            assert_eq!(m.save_state(), expected);
            m.load_state(&saved).unwrap();
        }
        if saw_address && saw_buffer && saw_nmi {
            break;
        }
    }
    assert!(
        saw_address && saw_buffer && saw_nmi,
        "missing replay checkpoint"
    );
}

#[test]
fn malformed_ppu_snapshots_fail_transactionally_including_old_snapshot_version() {
    let mut m = Machine::new(Cartridge::from_ines(&ppu_rom()).unwrap());
    m.run_instructions(50_000).unwrap();
    let before = m.save_state();
    for (pointer, value) in [
        ("/version", serde_json::json!(1)),
        ("/version", serde_json::json!(2)),
        ("/ppu/nametables", serde_json::json!([])),
        ("/ppu/oam", serde_json::json!([])),
        ("/ppu/palette/0", serde_json::json!(64)),
        ("/ppu/oam/2", serde_json::json!(0x1c)),
        ("/ppu/v", serde_json::json!(0x8000)),
        ("/ppu/fine_x", serde_json::json!(8)),
        ("/ppu/eval_n", serde_json::json!(64)),
        ("/ppu/total_dots", serde_json::json!(0)),
        ("/cpu/cycles", serde_json::json!(u64::MAX)),
        ("/dma_page", serde_json::json!(2)),
    ] {
        let mut bad: serde_json::Value = serde_json::from_slice(&before).unwrap();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            m.load_state(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{pointer}"
        );
        assert_eq!(m.save_state(), before);
    }
}
