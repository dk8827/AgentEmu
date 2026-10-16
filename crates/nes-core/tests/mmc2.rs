use nes_core::{
    cartridge::{Cartridge, Mirroring},
    machine::Machine,
};
fn image() -> Vec<u8> {
    let mut r = vec![0; 16 + 131072 + 131072];
    r[..4].copy_from_slice(b"NES\x1a");
    r[4] = 8;
    r[5] = 16;
    r[6] = 0x90;
    for bank in 0..16 {
        r[16 + bank * 8192..16 + (bank + 1) * 8192].fill(bank as u8);
    }
    for bank in 0..32 {
        r[16 + 131072 + bank * 4096..16 + 131072 + (bank + 1) * 4096].fill(bank as u8);
    }
    r
}
#[test]
fn switchable_8k_prg_fixed_tail_mirroring_and_no_prg_ram() {
    let mut c = Cartridge::from_ines(&image()).unwrap();
    c.cpu_write(0xafff, 0xfb, 0);
    assert_eq!(
        [0x8000, 0xa000, 0xc000, 0xe000].map(|a| c.cpu_read(a)),
        [Some(11), Some(13), Some(14), Some(15)]
    );
    c.cpu_write(0x6000, 42, 1);
    assert_eq!(c.cpu_read(0x6000), None);
    c.cpu_write(0xffff, 0, 2);
    assert_eq!(c.mirroring(), Mirroring::Vertical);
    c.cpu_write(0xf000, 3, 3);
    assert_eq!(c.mirroring(), Mirroring::Horizontal);
}
#[test]
fn read_latches_are_post_fetch_and_asymmetric_without_peek_or_write_side_effects() {
    let mut c = Cartridge::from_ines(&image()).unwrap();
    for (a, b) in [(0xb000, 1), (0xc000, 2), (0xd000, 3), (0xe000, 4)] {
        c.cpu_write(a, b, 0);
    }
    assert_eq!(c.chr_read(0x0fd8), 2); // triggering byte comes from old bank
    assert_eq!(c.chr_read(0), 2); // repeated debugger peeks do not latch
    c.chr_write(0x0fd8, 99);
    assert_eq!(c.chr_read(0), 2);
    c.ppu_read_complete(0x0fd9);
    assert_eq!(c.chr_read(0), 2);
    c.ppu_read_complete(0x0fd8);
    assert_eq!(c.chr_read(0), 1);
    c.ppu_read_complete(0x0fe9);
    assert_eq!(c.chr_read(0), 1);
    c.ppu_read_complete(0x0fe8);
    assert_eq!(c.chr_read(0), 2);
    for a in 0x1fd8..=0x1fdf {
        c.ppu_read_complete(a);
        assert_eq!(c.chr_read(0x1000), 3);
        c.ppu_read_complete(a + 16);
        assert_eq!(c.chr_read(0x1000), 4);
    }
}
#[test]
fn cpu_ppudata_reads_latch_and_snapshot_restores_mapper_state() {
    let mut r = image();
    // Wait past the PPU register startup inhibit, then configure two CHR banks,
    // read the trigger via PPUDATA and loop in the fixed PRG bank.
    let code = [
        0xa9, 1, 0x8d, 0, 0xb0, 0xa9, 2, 0x8d, 0, 0xc0, 0xad, 2, 0x20, 0xa9, 0x0f, 0x8d, 6, 0x20,
        0xa9, 0xd8, 0x8d, 6, 0x20, 0xad, 7, 0x20, 0x4c, 0x1a, 0xe0,
    ];
    r[16 + 15 * 8192..16 + 15 * 8192 + code.len()].copy_from_slice(&code);
    r[16 + 131072 - 4..16 + 131072 - 2].copy_from_slice(&0xe000u16.to_le_bytes());
    let mut m = Machine::new(Cartridge::from_ines(&r).unwrap());
    let mut state: serde_json::Value = serde_json::from_slice(&m.save_state()).unwrap();
    // Advance naturally instead of manipulating PPU state. The loop is reset to
    // execute the setup again after startup inhibition expires.
    m.run_frame().unwrap();
    m.run_frame().unwrap();
    m.cpu.pc = 0xe000;
    let before = m.save_state();
    m.run_instructions(14).unwrap();
    assert_eq!(m.peek_ppu(0), 1);
    let after = m.save_state();
    m.load_state(&before).unwrap();
    m.run_instructions(14).unwrap();
    assert_eq!(m.save_state(), after);
    state["cartridge"]["mapper"]["Mmc2"]["prg"] = 255.into();
    assert!(m.load_state(&serde_json::to_vec(&state).unwrap()).is_err());
}
