use nes_core::cartridge::{Cartridge, Mirroring};
fn rom(mapper: u8, prg: usize, chr: usize) -> Vec<u8> {
    let mut r = vec![0; 16 + prg * 16384 + chr * 8192];
    r[..4].copy_from_slice(b"NES\x1a");
    r[4] = prg as u8;
    r[5] = chr as u8;
    r[6] = mapper << 4;
    r[7] = mapper & 0xf0;
    for i in 0..prg {
        r[16 + i * 16384..16 + (i + 1) * 16384].fill(i as u8);
    }
    for i in 0..chr {
        r[16 + prg * 16384 + i * 8192..16 + prg * 16384 + (i + 1) * 8192].fill(i as u8);
    }
    r
}
fn serial(c: &mut Cartridge, addr: u16, value: u8) {
    for i in 0..5 {
        c.cpu_write(addr, (value >> i) & 1, 100 + i as u64 * 2);
    }
}
#[test]
fn cnrom_conflicts_switch_chr_without_switching_prg() {
    let mut r = rom(3, 2, 4);
    r[16] = 3;
    r[17] = 1;
    let mut c = Cartridge::from_ines(&r).unwrap();
    c.cpu_write(0x8000, 2, 0);
    assert_eq!(c.chr_read(0), 2);
    assert_eq!(c.cpu_read(0xc000), Some(1));
    c.cpu_write(0x8001, 3, 2);
    assert_eq!(c.chr_read(8191), 1);
    assert_eq!(c.cpu_read(0x6000), None);
}
#[test]
fn axrom_switches_whole_prg_and_single_screen_with_writable_chr() {
    let mut c = Cartridge::from_ines(&rom(7, 16, 0)).unwrap();
    c.cpu_write(0x8000, 0x13, 0);
    assert_eq!(c.cpu_read(0x8000), Some(6));
    assert_eq!(c.cpu_read(0xffff), Some(7));
    assert_eq!(c.mirroring(), Mirroring::OneScreenUpper);
    c.chr_write(123, 42);
    c.cpu_write(0x8000, 1, 1);
    assert_eq!(c.chr_read(123), 42);
    assert_eq!(c.mirroring(), Mirroring::OneScreenLower);
}
#[test]
fn surom_fixed_and_switchable_banks_follow_outer_region() {
    let mut c = Cartridge::from_ines(&rom(1, 32, 0)).unwrap();
    assert_eq!(c.cpu_read(0xc000), Some(15));
    serial(&mut c, 0xa000, 16);
    serial(&mut c, 0xe000, 3);
    assert_eq!(c.cpu_read(0x8000), Some(19));
    assert_eq!(c.cpu_read(0xc000), Some(31));
    serial(&mut c, 0x8000, 8);
    assert_eq!(c.cpu_read(0x8000), Some(16));
    assert_eq!(c.cpu_read(0xc000), Some(19));
    serial(&mut c, 0x8000, 0);
    assert_eq!(c.cpu_read(0x8000), Some(18));
    assert_eq!(c.cpu_read(0xc000), Some(19));
}
#[test]
fn fme7_banks_ram_mirroring_and_cpu_irq() {
    let mut c = Cartridge::from_ines(&rom(69, 16, 16)).unwrap();
    fn cmd(c: &mut Cartridge, n: u8, v: u8) {
        c.cpu_write(0x8000, n, 0);
        c.cpu_write(0xa000, v, 0);
    }
    cmd(&mut c, 9, 6);
    assert_eq!(c.cpu_read(0x8000), Some(3));
    assert_eq!(c.cpu_read(0xffff), Some(15));
    cmd(&mut c, 0, 8);
    assert_eq!(c.chr_read(0), 1);
    cmd(&mut c, 8, 0xc0);
    c.cpu_write(0x6000, 42, 0);
    assert_eq!(c.cpu_read(0x6000), Some(42));
    cmd(&mut c, 8, 0x40);
    assert_eq!(c.cpu_read(0x6000), None);
    cmd(&mut c, 12, 3);
    assert_eq!(c.mirroring(), Mirroring::OneScreenUpper);
    cmd(&mut c, 14, 1);
    cmd(&mut c, 15, 0);
    cmd(&mut c, 13, 0x81);
    c.clock_cpu();
    assert!(!c.irq());
    c.clock_cpu();
    assert!(c.irq());
    cmd(&mut c, 13, 0);
    assert!(!c.irq());
}
#[test]
fn mmc5_prg_modes_protection_multiplier_and_nametables() {
    let mut c = Cartridge::from_ines(&rom(5, 16, 16)).unwrap();
    c.cpu_write(0x5100, 2, 0);
    c.cpu_write(0x5115, 0x86, 0);
    c.cpu_write(0x5116, 0x89, 0);
    assert_eq!(c.cpu_read(0x8000), Some(3));
    assert_eq!(c.cpu_read(0xc000), Some(4));
    assert_eq!(c.cpu_read(0xffff), Some(15));
    c.cpu_write(0x6000, 42, 0);
    assert_eq!(c.cpu_read(0x6000), Some(0));
    c.cpu_write(0x5102, 2, 0);
    c.cpu_write(0x5103, 1, 0);
    c.cpu_write(0x6000, 42, 0);
    assert_eq!(c.cpu_read(0x6000), Some(42));
    c.cpu_write(0x5205, 255, 0);
    c.cpu_write(0x5206, 255, 0);
    assert_eq!(c.cpu_read(0x5205), Some(1));
    assert_eq!(c.cpu_read(0x5206), Some(254));
    c.cpu_write(0x5105, 0xe4, 0);
    c.cpu_write(0x5104, 0, 0);
    c.cpu_write(0x5106, 19, 0);
    c.cpu_write(0x5107, 2, 0);
    let mut nt = vec![0; 4096];
    c.nametable_write(0x2000, 8, &mut nt);
    c.nametable_write(0x2400, 9, &mut nt);
    c.nametable_write(0x2800, 10, &mut nt);
    assert_eq!(
        [0x2000, 0x2400, 0x2800, 0x2c00, 0x2fc0].map(|a| c.nametable_read(a, &nt)),
        [Some(8), Some(9), Some(10), Some(19), Some(0xaa)]
    );
}
#[test]
fn mmc5_background_sprite_banks_extended_attributes_and_irq_ack() {
    let mut c = Cartridge::from_ines(&rom(5, 16, 16)).unwrap();
    c.cpu_write(0x5120, 8, 0);
    c.cpu_write(0x5128, 16, 0);
    assert_eq!(c.render_chr(0, false, true, 0), 1);
    assert_eq!(c.render_chr(0, true, true, 0), 2);
    c.cpu_write(0x5104, 2, 0);
    c.cpu_write(0x5c05, 0xc6, 0);
    c.cpu_write(0x5104, 1, 0);
    assert_eq!(c.extended_attribute(5), Some(3));
    assert_eq!(c.render_chr(0, true, true, 5), 3);
    c.cpu_write(0x5203, 1, 0);
    c.cpu_write(0x5204, 128, 0);
    for a in [0x2000, 0x2000, 0x2000, 0x23c0] {
        c.ppu_read_complete(a);
    }
    assert_eq!(c.cpu_read(0x5204), Some(64));
    assert!(!c.irq());
    for a in [0x2000, 0x2000, 0x2000, 0x23c0] {
        c.ppu_read_complete(a);
    }
    assert!(c.irq());
    assert_eq!(c.cpu_read(0x5204), Some(192));
    c.cpu_read_complete(0x5204);
    assert!(!c.irq());
    for _ in 0..4 {
        c.clock_cpu();
    }
    assert_eq!(c.cpu_read(0x5204), Some(0));
}
#[test]
fn pal_has_312_lines_fractional_cpu_ratio_and_48k_audio_and_replay() {
    use nes_core::{diagnostic, machine::Machine};
    let mut r = diagnostic::audio_rom();
    r[9] = 1;
    let mut m = Machine::new(Cartridge::from_ines(&r).unwrap());
    assert!((m.frame_rate() - 50.007).abs() < 0.01);
    m.run_frame().unwrap();
    m.take_audio();
    let cycles = m.cpu.cycles;
    for _ in 0..50 {
        m.run_frame().unwrap();
    }
    let n = m.take_audio().len();
    assert!((47990..48020).contains(&n), "{n}");
    assert!((m.cpu.cycles - cycles).abs_diff(1_662_375) < 10);
    let saved = m.save_state();
    m.run_frame().unwrap();
    let expected = m.save_state();
    let sound = m.take_audio();
    m.load_state(&saved).unwrap();
    m.run_frame().unwrap();
    assert_eq!(m.save_state(), expected);
    assert_eq!(m.take_audio(), sound);
}
