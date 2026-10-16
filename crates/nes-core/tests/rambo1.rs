use nes_core::{
    cartridge::{Cartridge, Mirroring},
    machine::Machine,
};
fn image() -> Vec<u8> {
    let mut r = vec![0; 16 + 65536 + 32768];
    r[..4].copy_from_slice(b"NES\x1a");
    r[4] = 4;
    r[5] = 4;
    r[7] = 64;
    for i in 0..8 {
        r[16 + i * 8192..16 + (i + 1) * 8192].fill(i as u8);
    }
    for i in 0..32 {
        r[16 + 65536 + i * 1024..16 + 65536 + (i + 1) * 1024].fill(i as u8);
    }
    r
}
fn bank(c: &mut Cartridge, r: u8, v: u8) {
    c.cpu_write(0x8000, r, 0);
    c.cpu_write(0x8001, v, 2);
}
#[test]
fn three_switchable_prg_banks_both_orders_and_fixed_vector() {
    let mut c = Cartridge::from_ines(&image()).unwrap();
    bank(&mut c, 6, 2);
    bank(&mut c, 7, 3);
    bank(&mut c, 15, 5);
    let read = |c: &Cartridge| [0x8000, 0xa000, 0xc000, 0xe000].map(|a| c.cpu_read(a).unwrap());
    assert_eq!(read(&c), [2, 3, 5, 7]);
    c.cpu_write(0x8000, 64, 0);
    assert_eq!(read(&c), [5, 2, 3, 7]);
    bank(&mut c, 6, 255);
    assert_eq!(c.cpu_read(0x8000), Some(7));
    c.cpu_write(0x6000, 42, 0);
    c.cpu_write(0xa001, 128, 0);
    assert_eq!(c.cpu_read(0x6000), None);
    c.cpu_write(0xa000, 0, 0);
    assert_eq!(c.mirroring(), Mirroring::Vertical);
    c.cpu_write(0xa000, 1, 0);
    assert_eq!(c.mirroring(), Mirroring::Horizontal);
}
#[test]
fn chr_pairs_independent_banks_and_inversion() {
    let mut c = Cartridge::from_ines(&image()).unwrap();
    for (r, v) in [
        (0, 3),
        (1, 7),
        (2, 10),
        (3, 11),
        (4, 12),
        (5, 13),
        (8, 20),
        (9, 21),
    ] {
        bank(&mut c, r, v);
    }
    let read = |c: &Cartridge| [0, 1024, 2048, 3072, 4096, 5120, 6144, 7168].map(|a| c.chr_read(a));
    assert_eq!(read(&c), [2, 3, 6, 7, 10, 11, 12, 13]);
    c.cpu_write(0x8000, 32, 0);
    assert_eq!(read(&c), [3, 20, 7, 21, 10, 11, 12, 13]);
    c.cpu_write(0x8000, 160, 0);
    assert_eq!(read(&c), [10, 11, 12, 13, 3, 20, 7, 21]);
}
#[test]
fn cpu_irq_prescaler_reload_delay_ack_and_enable_does_not_ack() {
    let mut c = Cartridge::from_ines(&image()).unwrap();
    c.cpu_write(0xc000, 2, 0);
    c.cpu_write(0xc001, 1, 0);
    c.cpu_write(0xe001, 0, 0);
    for _ in 0..16 {
        c.clock_cpu();
        assert!(!c.irq());
    }
    c.clock_cpu();
    assert!(c.irq());
    c.cpu_write(0xe001, 0, 0);
    assert!(c.irq());
    c.cpu_write(0xe000, 0, 0);
    assert!(!c.irq());
    for _ in 0..20 {
        c.clock_cpu();
    }
    assert!(!c.irq());
}
#[test]
fn scanline_edges_filter_and_high_reload_does_not_raise_spurious_irq() {
    let mut c = Cartridge::from_ines(&image()).unwrap();
    c.cpu_write(0xc000, 1, 0);
    c.cpu_write(0xc001, 0, 0);
    c.cpu_write(0xe001, 0, 0);
    c.observe_ppu(0x1000, 30);
    c.observe_ppu(0, 31);
    c.observe_ppu(0x1000, 50);
    for _ in 0..3 {
        c.clock_cpu();
    }
    assert!(!c.irq());
    c.observe_ppu(0, 51);
    c.observe_ppu(0x1000, 90);
    c.clock_cpu();
    assert!(!c.irq());
    c.clock_cpu();
    assert!(c.irq());
    c.cpu_write(0xe000, 0, 0);
    c.cpu_write(0xc000, 254, 0);
    c.cpu_write(0xc001, 0, 0);
    c.cpu_write(0xe001, 0, 0);
    c.observe_ppu(0, 91);
    c.observe_ppu(0x1000, 130);
    for _ in 0..3 {
        c.clock_cpu();
    }
    assert!(!c.irq());
}
#[test]
fn mapper_snapshot_replays_cpu_divider_and_banks() {
    let mut r = image();
    r[16 + 7 * 8192..16 + 7 * 8192 + 3].copy_from_slice(&[0x4c, 0, 0xe0]);
    r[16 + 65532..16 + 65534].copy_from_slice(&0xe000u16.to_le_bytes());
    let mut c = Cartridge::from_ines(&r).unwrap();
    bank(&mut c, 15, 4);
    c.cpu_write(0xc000, 100, 0);
    c.cpu_write(0xc001, 1, 0);
    let mut m = Machine::new(c);
    m.run_instructions(19).unwrap();
    m.take_audio();
    let save = m.save_state();
    m.run_frame().unwrap();
    let expected = m.save_state();
    let pcm = m.take_audio();
    m.load_state(&save).unwrap();
    m.run_frame().unwrap();
    assert_eq!(m.save_state(), expected);
    assert_eq!(m.take_audio(), pcm);
}
#[test]
fn camerica_ignores_bus_conflicts_and_only_firehawk_address_changes_mirroring() {
    let mut r = vec![0; 16 + 4 * 16384];
    r[..4].copy_from_slice(b"NES\x1a");
    r[4] = 4;
    r[6] = 0x71;
    r[7] = 0x40;
    for i in 0..4 {
        r[16 + i * 16384..16 + (i + 1) * 16384].fill(i as u8);
    }
    let mut c = Cartridge::from_ines(&r).unwrap();
    c.cpu_write(0xc000, 2, 0);
    assert_eq!(c.cpu_read(0x8000), Some(2));
    assert_eq!(c.cpu_read(0xffff), Some(3));
    c.cpu_write(0x8000, 0, 0);
    assert_eq!(c.cpu_read(0x8000), Some(2));
    assert_eq!(c.mirroring(), Mirroring::Vertical);
    c.cpu_write(0x9000, 16, 0);
    assert_eq!(c.mirroring(), Mirroring::OneScreenUpper);
    c.cpu_write(0x6000, 42, 0);
    assert_eq!(c.cpu_read(0x6000), None);
    c.chr_write(123, 77);
    assert_eq!(c.chr_read(123), 77);
}
