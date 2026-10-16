use nes_core::{
    cartridge::Cartridge,
    diagnostic::controller_rom,
    ppu::{DOTS_PER_FRAME, Ppu, WRITE_INHIBIT_DOTS},
};

fn ready() -> (Ppu, Cartridge) {
    let mut ppu = Ppu::default();
    let cartridge = Cartridge::from_ines(&controller_rom()).unwrap();
    for _ in 0..WRITE_INHIBIT_DOTS {
        ppu.tick(&cartridge);
    }
    (ppu, cartridge)
}
fn write(p: &mut Ppu, c: &mut Cartridge, reg: u16, value: u8) {
    p.write_register(c, reg, value).unwrap();
}
fn address(p: &mut Ppu, c: &mut Cartridge, addr: u16) {
    p.read_register(c, 0x2002);
    write(p, c, 0x2006, (addr >> 8) as u8);
    write(p, c, 0x2006, addr as u8);
}
fn poke(p: &mut Ppu, c: &mut Cartridge, addr: u16, value: u8) {
    address(p, c, addr);
    write(p, c, 0x2007, value);
}
fn advance_to(p: &mut Ppu, c: &Cartridge, scanline: u16, dot: u16) {
    for _ in 0..=DOTS_PER_FRAME {
        if (p.scanline(), p.dot()) == (scanline, dot) {
            return;
        }
        p.tick(c);
    }
    panic!("PPU did not reach coordinate");
}

#[test]
fn startup_inhibits_only_control_mask_scroll_address_and_still_refreshes_bus() {
    let mut p = Ppu::default();
    let mut c = Cartridge::from_ines(&controller_rom()).unwrap();
    for reg in [0x2000, 0x2001, 0x2005, 0x2006] {
        write(&mut p, &mut c, reg, 0xff);
        assert_eq!(p.read_register(&c, 0x2000), 0xff);
    }
    write(&mut p, &mut c, 0x2007, 0x42); // address writes were ignored: CHR $0000
    assert_eq!(p.peek_memory(&c, 0), 0x42);
    write(&mut p, &mut c, 0x2003, 7);
    write(&mut p, &mut c, 0x2004, 0xab);
    write(&mut p, &mut c, 0x2003, 7);
    assert_eq!(p.read_register(&c, 0x2004), 0xab);
    for _ in 0..WRITE_INHIBIT_DOTS {
        p.tick(&c);
    }
    write(&mut p, &mut c, 0x2001, 0x18);
    write(&mut p, &mut c, 0x2001, 0);
    address(&mut p, &mut c, 0x2123);
    write(&mut p, &mut c, 0x2007, 0x56);
    assert_eq!(p.peek_memory(&c, 0x2123), 0x56);
}

#[test]
fn chr_and_nametable_reads_are_delayed_palette_reads_refill_shadow_buffer() {
    let (mut p, mut c) = ready();
    poke(&mut p, &mut c, 0x100, 0xab);
    poke(&mut p, &mut c, 0x101, 0xcd);
    address(&mut p, &mut c, 0x100);
    assert_eq!(p.read_register(&c, 0x2007), 0);
    assert_eq!(p.read_register(&c, 0x2007), 0xab);
    assert_eq!(p.read_register(&c, 0x2007), 0xcd);
    poke(&mut p, &mut c, 0x2f00, 0x58);
    poke(&mut p, &mut c, 0x3f10, 0xff); // universal-color alias, only six bits stored
    address(&mut p, &mut c, 0x3f00);
    write(&mut p, &mut c, 0x2002, 0xc0); // read-only writes refresh PPU I/O latch
    assert_eq!(p.read_register(&c, 0x2007), 0xff);
    address(&mut p, &mut c, 0x100);
    assert_eq!(p.read_register(&c, 0x2007), 0x58); // shadowed nametable, no priming
    write(&mut p, &mut c, 0x2001, 1);
    address(&mut p, &mut c, 0x3f00);
    assert_eq!(p.read_register(&c, 0x2007), 0x30);
    assert_eq!(p.peek_memory(&c, 0x3f00), 0x3f); // grayscale doesn't alter storage
}

#[test]
fn palette_aliases_address_wrapping_and_stride_32() {
    let (mut p, mut c) = ready();
    for (i, addr) in [0x3f10, 0x3f14, 0x3f18, 0x3f1c].into_iter().enumerate() {
        poke(&mut p, &mut c, addr, 0x10 + i as u8);
        assert_eq!(p.peek_memory(&c, addr - 16), 0x10 + i as u8);
        assert_eq!(p.peek_memory(&c, addr + 0x20), 0x10 + i as u8);
    }
    poke(&mut p, &mut c, 0x3f04, 7);
    assert_eq!(p.peek_memory(&c, 0x3f00), 0x10); // other color-zero entries are distinct
    address(&mut p, &mut c, 0x3fff);
    write(&mut p, &mut c, 0x2007, 9);
    write(&mut p, &mut c, 0x2007, 0x91);
    assert_eq!(p.peek_memory(&c, 0), 0x91); // v's bit 14 is not on memory bus
    write(&mut p, &mut c, 0x2000, 4);
    address(&mut p, &mut c, 0x2100);
    for value in [1, 2, 3] {
        write(&mut p, &mut c, 0x2007, value);
    }
    for (addr, value) in [(0x2100, 1), (0x2120, 2), (0x2140, 3)] {
        assert_eq!(p.peek_memory(&c, addr), value);
    }
}

#[test]
fn horizontal_vertical_and_mapper_controlled_single_screen_mirroring() {
    for vertical in [false, true] {
        let (mut p, _) = ready();
        let mut bytes = controller_rom();
        bytes[6] = u8::from(vertical);
        let mut c = Cartridge::from_ines(&bytes).unwrap();
        poke(&mut p, &mut c, 0x2000, 0x42);
        poke(&mut p, &mut c, if vertical { 0x2400 } else { 0x2800 }, 0x99);
        let expected = if vertical {
            [0x42, 0x99, 0x42, 0x99]
        } else {
            [0x42, 0x42, 0x99, 0x99]
        };
        for (i, value) in expected.into_iter().enumerate() {
            assert_eq!(p.peek_memory(&c, 0x2000 + i as u16 * 1024), value);
            assert_eq!(p.peek_memory(&c, 0x3000 + i as u16 * 1024), value);
        }
    }
    let (mut p, _) = ready();
    let mut bytes = controller_rom();
    bytes[6] = 0x10;
    let mut c = Cartridge::from_ines(&bytes).unwrap();
    poke(&mut p, &mut c, 0x2000, 0x55);
    for bit in 0..5 {
        // select upper one-screen CIRAM page
        c.cpu_write(0x8000, (0x0d >> bit) & 1, bit as u64 * 2);
    }
    assert_eq!(p.peek_memory(&c, 0x2000), 0);
    poke(&mut p, &mut c, 0x2400, 0xaa);
    for addr in [0x2000, 0x2400, 0x2800, 0x2c00] {
        assert_eq!(p.peek_memory(&c, addr), 0xaa);
    }
    for bit in 0..5 {
        c.cpu_write(0x8000, (0x0c >> bit) & 1, 10 + bit as u64 * 2);
    }
    assert_eq!(p.peek_memory(&c, 0x2800), 0x55);
}

#[test]
fn scroll_and_address_share_latch_status_resets_it_and_registers_mirror() {
    let (mut p, mut c) = ready();
    write(&mut p, &mut c, 0x2000, 0);
    write(&mut p, &mut c, 0x2005, 0x7d);
    write(&mut p, &mut c, 0x2005, 0x5e);
    let state = serde_json::to_value(&p).unwrap();
    assert_eq!(state["t"], 0x616f); // NESdev worked scroll example
    assert_eq!(state["fine_x"], 5);
    write(&mut p, &mut c, 0x3ffe, 0x3d);
    write(&mut p, &mut c, 0x2006, 0xf0);
    write(&mut p, &mut c, 0x3fff, 0x42);
    assert_eq!(p.peek_memory(&c, 0x3df0), 0x42);
    write(&mut p, &mut c, 0x2005, 0); // first write
    write(&mut p, &mut c, 0x2006, 0x12); // shared second write: sets low byte
    assert_eq!(serde_json::to_value(&p).unwrap()["v"], 0x3d12);
    write(&mut p, &mut c, 0x2006, 0xff);
    p.read_register(&c, 0x3ffa); // mirrored status clears write toggle
    write(&mut p, &mut c, 0x2006, 0x21);
    write(&mut p, &mut c, 0x2006, 0x23);
    write(&mut p, &mut c, 0x2007, 0x88);
    assert_eq!(p.peek_memory(&c, 0x2123), 0x88);
}

#[test]
fn oam_writes_wrap_reads_do_not_increment_and_attribute_bits_are_unwired() {
    let (mut p, mut c) = ready();
    write(&mut p, &mut c, 0x2003, 0xff);
    write(&mut p, &mut c, 0x2004, 0x42);
    write(&mut p, &mut c, 0x2004, 0x99);
    write(&mut p, &mut c, 0x2003, 0xff);
    assert_eq!(p.read_register(&c, 0x2004), 0x42);
    assert_eq!(p.read_register(&c, 0x2004), 0x42);
    write(&mut p, &mut c, 0x2003, 0);
    assert_eq!(p.read_register(&c, 0x2004), 0x99);
    write(&mut p, &mut c, 0x2003, 2);
    write(&mut p, &mut c, 0x2004, 0xff);
    write(&mut p, &mut c, 0x2003, 2);
    assert_eq!(p.read_register(&c, 0x2004), 0xe3);
}

#[test]
fn vblank_nmi_level_and_status_peek_are_distinct_from_destructive_reads() {
    let (mut p, mut c) = ready();
    advance_to(&mut p, &c, 241, 1);
    assert_eq!(p.peek_register(&c, 0x2002) & 0x80, 0);
    p.tick(&c);
    assert_eq!(p.peek_register(&c, 0x2002) & 0x80, 0x80);
    assert!(!p.nmi_line());
    write(&mut p, &mut c, 0x2000, 0x80);
    assert!(p.nmi_line());
    write(&mut p, &mut c, 0x2000, 0);
    assert!(!p.nmi_line());
    write(&mut p, &mut c, 0x2000, 0x80);
    assert!(p.nmi_line());
    write(&mut p, &mut c, 0x2002, 0x1b);
    assert_eq!(p.peek_register(&c, 0x2002), 0x9b);
    assert_eq!(p.read_register(&c, 0x2002), 0x9b);
    assert!(!p.nmi_line());
    assert_eq!(p.read_register(&c, 0x2002), 0x1b);
    advance_to(&mut p, &c, 241, 1);
    p.tick(&c);
    assert!(p.nmi_line());
    advance_to(&mut p, &c, 261, 1);
    p.tick(&c);
    assert!(!p.nmi_line());
}

#[test]
fn status_read_immediately_before_vblank_suppresses_flag_until_next_frame() {
    for dot in [1] {
        let (mut p, mut c) = ready();
        write(&mut p, &mut c, 0x2000, 0x80);
        advance_to(&mut p, &c, 241, dot);
        assert_eq!(p.read_register(&c, 0x2002) & 0x80, 0);
        for _ in dot..=1 {
            p.tick(&c);
        }
        assert_eq!(p.peek_register(&c, 0x2002) & 0x80, 0);
        assert!(!p.nmi_line());
        advance_to(&mut p, &c, 241, 1);
        p.tick(&c);
        assert!(p.nmi_line());
    }
}

#[test]
fn disabled_rendering_keeps_both_frame_parities_at_89342_dots() {
    let mut p = Ppu::default();
    let c = Cartridge::from_ines(&controller_rom()).unwrap();
    for frame in 1..=3 {
        for _ in 0..DOTS_PER_FRAME {
            p.tick(&c);
        }
        assert_eq!((p.frame(), p.scanline(), p.dot()), (frame, 0, 0));
    }
}

#[test]
fn chr_rom_cannot_be_modified_through_cpu_ppudata_and_peeks_do_not_prime_reads() {
    let (mut p, _) = ready();
    let mut bytes = controller_rom();
    bytes[5] = 1;
    bytes.extend(vec![0x77; 8192]);
    let mut c = Cartridge::from_ines(&bytes).unwrap();
    poke(&mut p, &mut c, 0x1234, 0x99);
    address(&mut p, &mut c, 0x1234);
    assert_eq!(p.peek_memory(&c, 0x1234), 0x77);
    assert_eq!(p.peek_register(&c, 0x2007), 0);
    assert_eq!(p.peek_register(&c, 0x2007), 0);
    assert_eq!(p.read_register(&c, 0x2007), 0);
    assert_eq!(p.read_register(&c, 0x2007), 0x77);
}

#[test]
fn open_bus_decays_without_refresh_from_write_only_or_undriven_status_bits() {
    use nes_core::ppu::IO_DECAY_DOTS;
    let (mut p, mut c) = ready();
    write(&mut p, &mut c, 0x2002, 0xff);
    for _ in 0..IO_DECAY_DOTS / 2 {
        p.tick(&c);
    }
    assert_eq!(p.read_register(&c, 0x2000), 0xff);
    assert_eq!(p.read_register(&c, 0x2002) & 31, 31);
    for _ in 0..IO_DECAY_DOTS / 2 {
        p.tick(&c);
    }
    assert_eq!(p.peek_register(&c, 0x2000) & 31, 0);
    write(&mut p, &mut c, 0x2002, 0xa5);
    assert_eq!(p.read_register(&c, 0x2000), 0xa5);
}
