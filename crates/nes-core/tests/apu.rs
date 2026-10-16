use nes_core::{apu::Apu, cartridge::Cartridge, diagnostic::controller_rom, machine::Machine};
fn ticks(a: &mut Apu, n: usize) {
    for _ in 0..n {
        a.tick();
    }
}
fn state(a: &Apu) -> serde_json::Value {
    serde_json::to_value(a).unwrap()
}
#[test]
fn length_enable_halt_and_frame_irq_acknowledgement() {
    let mut a = Apu::default();
    a.write(0x4003, 0xf8);
    assert_eq!(a.status() & 1, 0);
    a.write(0x4015, 15);
    for r in [0x4003, 0x4007, 0x400b, 0x400f] {
        a.write(r, 0x18);
    }
    assert_eq!(a.status() & 15, 15);
    ticks(&mut a, 14913);
    assert_eq!(a.status() & 15, 15);
    ticks(&mut a, 14917);
    assert_eq!(a.status() & 15, 0);
    assert!(a.irq());
    assert_ne!(a.read_status() & 64, 0);
    assert!(!a.irq());
    a.write(0x4017, 0xc0);
    a.write(0x4000, 0x20);
    a.write(0x4003, 0x18);
    ticks(&mut a, 100000);
    assert_eq!(a.status() & 1, 1);
    assert!(!a.irq());
    a.write(0x4015, 0);
    assert_eq!(a.status() & 15, 0);
}
#[test]
fn envelope_and_triangle_linear_counter_reload_independently() {
    let mut a = Apu::default();
    a.write(0x4015, 5);
    a.write(0x4000, 0);
    a.write(0x4003, 8);
    a.write(0x4008, 2);
    a.write(0x400b, 8);
    ticks(&mut a, 7457);
    assert_eq!(state(&a)["pulse"][0]["envelope"]["decay"], 15);
    assert_eq!(state(&a)["triangle"]["linear"], 2);
    ticks(&mut a, 7456);
    assert_eq!(state(&a)["pulse"][0]["envelope"]["decay"], 14);
    assert_eq!(state(&a)["triangle"]["linear"], 1);
    ticks(&mut a, 7458);
    assert_eq!(state(&a)["triangle"]["linear"], 0);
    assert_ne!(a.status() & 4, 0);
}
#[test]
fn pulse_negation_differs_by_one_and_sweep_muting_is_independent_of_enable() {
    let mut a = Apu::default();
    a.write(0x4015, 3);
    for base in [0x4000, 0x4004] {
        a.write(base, 0x3f);
        a.write(base + 1, 0x89);
        a.write(base + 2, 100);
        a.write(base + 3, 8);
    }
    ticks(&mut a, 14913);
    assert_eq!(state(&a)["pulse"][0]["period"], 49);
    assert_eq!(state(&a)["pulse"][1]["period"], 50);
}
#[test]
fn dmc_buffer_address_wrap_loop_irq_and_status() {
    let mut a = Apu::default();
    a.write(0x4010, 0x8f);
    a.write(0x4012, 255);
    a.write(0x4013, 4);
    a.write(0x4015, 16);
    let mut addresses = vec![];
    for _ in 0..100000 {
        a.tick();
        if let Some(addr) = a.dmc_request() {
            addresses.push(addr);
            a.complete_dmc(0xff);
        }
        if a.status() & 128 != 0 {
            break;
        }
    }
    assert_eq!(addresses.len(), 65);
    assert_eq!(addresses[0], 0xffc0);
    assert_eq!(addresses[63], 0xffff);
    assert_eq!(addresses[64], 0x8000);
    assert_eq!(a.read_status() & 0x90, 0x80);
    assert!(a.irq());
    a.write(0x4015, 0);
    assert!(!a.irq());
    a.write(0x4010, 0xcf);
    a.write(0x4013, 0);
    a.write(0x4015, 16);
    ticks(&mut a, 500);
    if a.dmc_request().is_some() {
        a.complete_dmc(0);
    }
    assert!(!a.irq());
    assert_ne!(a.status() & 16, 0);
}
#[test]
fn audio_has_expected_rate_is_bounded_and_replays_after_snapshot() {
    let mut a = Apu::default();
    a.write(0x4015, 1);
    a.write(0x4000, 0xbf);
    a.write(0x4001, 8);
    a.write(0x4002, 253);
    a.write(0x4003, 8);
    ticks(&mut a, 1789773);
    let audio = a.take_samples();
    assert_eq!(audio.len(), 48000);
    assert!(audio.iter().all(|v| v.is_finite() && v.abs() < 1.0));
    assert!(audio.iter().any(|v| *v > 0.05));
    let saved = serde_json::to_vec(&a).unwrap();
    ticks(&mut a, 40000);
    let expected = a.take_samples();
    let mut restored: Apu = serde_json::from_slice(&saved).unwrap();
    ticks(&mut restored, 40000);
    assert_eq!(restored.take_samples(), expected);
}
#[test]
fn dmc_dma_stalls_cpu_reads_and_clocks_ppu_and_apu() {
    let mut rom = controller_rom();
    let code = [
        0xa9, 0x8f, 0x8d, 0x10, 0x40, 0xa9, 0, 0x8d, 0x13, 0x40, 0xa9, 16, 0x8d, 0x15, 0x40, 0xea,
        0xea, 0xea,
    ];
    rom[16..16 + code.len()].copy_from_slice(&code);
    let mut m = Machine::new(Cartridge::from_ines(&rom).unwrap());
    m.run_instructions(6).unwrap();
    let before = m.cpu.cycles;
    m.run_instructions(3).unwrap();
    assert!((9..=10).contains(&(m.cpu.cycles - before)));
    assert_eq!(m.ppu().total_dots(), m.cpu.cycles * 3);
    assert_eq!(m.apu().cycles(), m.cpu.cycles);
    assert_ne!(m.apu().status() & 128, 0);
}
#[test]
fn malformed_audio_state_is_rejected_transactionally() {
    let mut m = Machine::new(Cartridge::from_ines(&controller_rom()).unwrap());
    m.run_instructions(100).unwrap();
    let before = m.save_state();
    for (pointer, value) in [
        ("/apu/noise/shift", serde_json::json!(0)),
        ("/apu/pulse/0/phase", serde_json::json!(8)),
        ("/apu/dmc/bits", serde_json::json!(0)),
        ("/apu/sample_count", serde_json::json!(99)),
    ] {
        let mut bad: serde_json::Value = serde_json::from_slice(&before).unwrap();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(m.load_state(&serde_json::to_vec(&bad).unwrap()).is_err());
        assert_eq!(m.save_state(), before);
    }
}

#[test]
fn dmc_setup_overlaps_oam_dma_and_steals_only_two_cycles_mid_transfer() {
    let mut rom = controller_rom();
    rom[16..22].copy_from_slice(&[0xa9, 2, 0x8d, 0x14, 0x40, 0xea]);
    let mut elapsed = Vec::new();
    for enabled in [false, true] {
        let mut m = Machine::new(Cartridge::from_ines(&rom).unwrap());
        m.step().unwrap(); // LDA #2; next instruction starts OAM DMA
        let mut state: serde_json::Value = serde_json::from_slice(&m.save_state()).unwrap();
        state["apu"]["dmc"]["buffer"] = serde_json::json!(0x55);
        state["apu"]["dmc"]["bits"] = serde_json::json!(1);
        state["apu"]["dmc"]["timer"] = serde_json::json!(100);
        state["apu"]["dmc"]["remaining"] = serde_json::json!(u8::from(enabled));
        m.load_state(&serde_json::to_vec(&state).unwrap()).unwrap();
        elapsed.push(m.step().unwrap());
        assert_eq!(m.ppu().total_dots(), m.cpu.cycles * 3);
        assert_eq!(m.apu().cycles(), m.cpu.cycles);
    }
    assert_eq!(elapsed[1], elapsed[0] + 2);
}

#[test]
fn dmc_collision_loses_one_joypad_bit_not_one_per_dummy_cycle() {
    for port in [0x16, 0x17] {
        let mut rom = controller_rom();
        for i in 0..10 {
            rom[16 + i * 3..19 + i * 3].copy_from_slice(&[0xad, port, 0x40]);
        }
        let mut m = Machine::new(Cartridge::from_ines(&rom).unwrap());
        let mut state: serde_json::Value = serde_json::from_slice(&m.save_state()).unwrap();
        state["apu"]["dmc"]["buffer"] = serde_json::json!(0x55);
        state["apu"]["dmc"]["bits"] = serde_json::json!(1);
        state["apu"]["dmc"]["timer"] = serde_json::json!(2);
        state["apu"]["dmc"]["remaining"] = serde_json::json!(1);
        m.load_state(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(m.step().unwrap() > 4); // final $4016/17 read collided with DMA
        assert_eq!(m.cpu.a & 1, 0);
        for _ in 0..6 {
            m.step().unwrap();
            assert_eq!(m.cpu.a & 1, 0);
        }
        m.step().unwrap();
        assert_eq!(m.cpu.a & 1, 1);
    }
}
