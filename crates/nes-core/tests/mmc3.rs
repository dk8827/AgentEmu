use nes_core::{
    cartridge::{Cartridge, Mirroring},
    machine::Machine,
    ppu::{Ppu, WRITE_INHIBIT_DOTS},
};
fn image(four: bool) -> Vec<u8> {
    let mut r = vec![0; 16 + 65536 + 32768];
    r[..4].copy_from_slice(b"NES\x1a");
    r[4] = 4;
    r[5] = 4;
    r[6] = 0x40 | if four { 8 } else { 0 };
    for bank in 0..8 {
        r[16 + bank * 8192..16 + (bank + 1) * 8192].fill(bank as u8);
    }
    for bank in 0..32 {
        r[16 + 65536 + bank * 1024..16 + 65536 + (bank + 1) * 1024].fill(bank as u8);
    }
    r
}
fn bank(c: &mut Cartridge, select: u8, value: u8) {
    c.cpu_write(0x8000, select, 0);
    c.cpu_write(0x8001, value, 2);
}
#[test]
fn prg_modes_chr_inversion_even_pairs_and_bank_masks() {
    let mut c = Cartridge::from_ines(&image(false)).unwrap();
    bank(&mut c, 6, 3);
    bank(&mut c, 7, 4);
    assert_eq!(
        [0x8000, 0xa000, 0xc000, 0xe000].map(|a| c.cpu_read(a).unwrap()),
        [3, 4, 6, 7]
    );
    c.cpu_write(0x8000, 0x46, 4);
    assert_eq!(
        [0x8000, 0xa000, 0xc000, 0xe000].map(|a| c.cpu_read(a).unwrap()),
        [6, 4, 3, 7]
    );
    bank(&mut c, 6, 255);
    assert_eq!(c.cpu_read(0x8000), Some(7));
    for (i, v) in [3, 7, 9, 11, 13, 15].into_iter().enumerate() {
        bank(&mut c, i as u8, v);
    }
    assert_eq!(
        [0, 1024, 2048, 3072, 4096, 5120, 6144, 7168].map(|a| c.chr_read(a)),
        [2, 3, 6, 7, 9, 11, 13, 15]
    );
    c.cpu_write(0x8000, 0x80, 0);
    assert_eq!(
        [0, 1024, 2048, 3072, 4096, 5120, 6144, 7168].map(|a| c.chr_read(a)),
        [9, 11, 13, 15, 2, 3, 6, 7]
    );
}
#[test]
fn ram_protection_mirroring_and_four_screen_override() {
    let mut c = Cartridge::from_ines(&image(false)).unwrap();
    c.cpu_write(0x6000, 42, 0);
    c.cpu_write(0xa001, 0xc0, 0);
    c.cpu_write(0x6000, 99, 0);
    assert_eq!(c.cpu_read(0x6000), Some(42));
    c.cpu_write(0xa001, 0, 0);
    assert_eq!(c.cpu_read(0x6000), None);
    c.cpu_write(0xa001, 0x80, 0);
    assert_eq!(c.cpu_read(0x6000), Some(42));
    c.cpu_write(0xa000, 0, 0);
    assert_eq!(c.mirroring(), Mirroring::Vertical);
    c.cpu_write(0xa000, 1, 0);
    assert_eq!(c.mirroring(), Mirroring::Horizontal);
    let mut c = Cartridge::from_ines(&image(true)).unwrap();
    c.cpu_write(0xa000, 1, 0);
    assert_eq!(c.mirroring(), Mirroring::FourScreen);
    let mut p = Ppu::default();
    for _ in 0..WRITE_INHIBIT_DOTS {
        p.tick(&c);
    }
    for i in 0..4 {
        p.write_register(&mut c, 0x2006, 0x20 + i * 4).unwrap();
        p.write_register(&mut c, 0x2006, 0).unwrap();
        p.write_register(&mut c, 0x2007, 10 + i).unwrap();
    }
    assert_eq!(
        [0x2000, 0x2400, 0x2800, 0x2c00].map(|a| p.peek_memory(&c, a)),
        [10, 11, 12, 13]
    );
}
#[test]
fn filtered_edges_reload_disable_ack_and_both_irq_revisions() {
    for alternate in [false, true] {
        let mut c = Cartridge::from_ines(&image(false)).unwrap();
        c.set_mmc3_alternate(alternate).unwrap();
        c.cpu_write(0xc000, 1, 0);
        c.cpu_write(0xc001, 0, 0);
        c.cpu_write(0xe001, 0, 0);
        c.observe_ppu(0, 0);
        c.observe_ppu(0x1000, 10);
        assert!(!c.irq());
        c.observe_ppu(0, 11);
        c.observe_ppu(0x1000, 20);
        assert!(!c.irq());
        c.observe_ppu(0, 21);
        c.observe_ppu(0x1000, 31);
        assert!(c.irq());
        c.cpu_write(0xe000, 0, 0);
        assert!(!c.irq());
        c.cpu_write(0xe001, 0, 0);
        c.cpu_write(0xc000, 0, 0);
        c.observe_ppu(0, 32);
        c.observe_ppu(0x1000, 42);
        assert_eq!(c.irq(), !alternate);
        c.cpu_write(0xe000, 0, 0);
        c.cpu_write(0xe001, 0, 0);
        c.cpu_write(0xc001, 0, 0);
        c.observe_ppu(0, 43);
        c.observe_ppu(0x1000, 53);
        assert!(c.irq());
    }
}
#[test]
fn chr_ram_banking_and_snapshot_preserve_filter_phase() {
    let mut r = image(false);
    r[5] = 0;
    r.truncate(16 + 65536);
    r[16 + 7 * 8192..16 + 7 * 8192 + 3].copy_from_slice(&[0x4c, 0, 0xe0]);
    r[16 + 65532..16 + 65534].copy_from_slice(&0xe000u16.to_le_bytes());
    let mut c = Cartridge::from_ines(&r).unwrap();
    bank(&mut c, 2, 3);
    c.chr_write(0x1000, 77);
    bank(&mut c, 2, 4);
    assert_eq!(c.chr_read(0x1000), 0);
    bank(&mut c, 2, 3);
    assert_eq!(c.chr_read(0x1000), 77);
    let mut m = Machine::new(c);
    m.run_instructions(20).unwrap();
    let save = m.save_state();
    m.run_frame().unwrap();
    let expected = m.save_state();
    m.load_state(&save).unwrap();
    m.run_frame().unwrap();
    assert_eq!(m.save_state(), expected);
}
