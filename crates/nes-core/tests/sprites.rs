use nes_core::{
    cartridge::Cartridge,
    diagnostic::background_rom,
    ppu::{DOTS_PER_FRAME, Ppu, WRITE_INHIBIT_DOTS},
};

fn write(p: &mut Ppu, c: &mut Cartridge, r: u16, v: u8) {
    p.write_register(c, r, v).unwrap();
}
fn addr(p: &mut Ppu, c: &mut Cartridge, a: u16) {
    p.read_register(c, 0x2002);
    write(p, c, 0x2006, (a >> 8) as u8);
    write(p, c, 0x2006, a as u8);
}
fn advance(p: &mut Ppu, c: &Cartridge, line: u16, dot: u16) {
    for _ in 0..=DOTS_PER_FRAME {
        if (p.scanline(), p.dot()) == (line, dot) {
            return;
        }
        p.tick(c);
    }
    panic!("coordinate not reached")
}
fn finish(p: &mut Ppu, c: &Cartridge) {
    let end = p.completed_frames() + 2;
    while p.completed_frames() < end {
        p.tick(c);
    }
}
fn scene(entries: &[[u8; 4]], ctrl: u8, mask: u8) -> (Ppu, Cartridge) {
    let mut rom = background_rom();
    // A tile with a unique two-bit color at each coordinate; bank one differs.
    for bank in 0..2 {
        for tile in 0..256 {
            for y in 0..8 {
                let mut low = 0;
                let mut high = 0;
                for x in 0..8 {
                    let color = color(bank, tile, x, y);
                    low |= (color & 1) << (7 - x);
                    high |= (color >> 1) << (7 - x);
                }
                let i = 16 + 16384 + bank * 4096 + tile * 16 + y;
                rom[i] = low;
                rom[i + 8] = high;
            }
        }
    }
    let mut c = Cartridge::from_ines(&rom).unwrap();
    let mut p = Ppu::default();
    for _ in 0..WRITE_INHIBIT_DOTS {
        p.tick(&c);
    }
    addr(&mut p, &mut c, 0x3f00);
    for i in 0..32 {
        write(&mut p, &mut c, 0x2007, if i % 4 == 0 { 0 } else { i });
    }
    write(&mut p, &mut c, 0x2003, 0);
    for _ in 0..64 {
        for v in [255, 0, 0, 0] {
            write(&mut p, &mut c, 0x2004, v);
        }
    }
    write(&mut p, &mut c, 0x2003, 0);
    for entry in entries {
        for &v in entry {
            write(&mut p, &mut c, 0x2004, v);
        }
    }
    write(&mut p, &mut c, 0x2003, 0);
    addr(&mut p, &mut c, 0x2000);
    for _ in 0..960 {
        write(&mut p, &mut c, 0x2007, 0);
    }
    for _ in 0..64 {
        write(&mut p, &mut c, 0x2007, 0);
    }
    write(&mut p, &mut c, 0x2000, ctrl);
    write(&mut p, &mut c, 0x2005, 0);
    write(&mut p, &mut c, 0x2005, 0);
    write(&mut p, &mut c, 0x2001, mask);
    (p, c)
}
fn color(bank: usize, tile: usize, x: usize, y: usize) -> u8 {
    ((x + 2 * y + tile + bank) % 4) as u8
}
#[test]
fn sprite_sizes_flips_palette_and_y_offset_match_geometric_oracle() {
    for height in [8, 16] {
        for flip in [0, 0x40, 0x80, 0xc0] {
            let ctrl = if height == 16 { 0x20 } else { 8 };
            let (mut p, c) = scene(&[[19, 3, flip | 2, 40]], ctrl, 0x14);
            finish(&mut p, &c);
            for y in 0..240 {
                for x in 0..256 {
                    let expected = if (20..20 + height).contains(&y) && (40..48).contains(&x) {
                        let px = if flip & 0x40 != 0 { 47 - x } else { x - 40 };
                        let py = if flip & 0x80 != 0 {
                            height - 1 - (y - 20)
                        } else {
                            y - 20
                        };
                        let tile = if height == 16 { 2 + py / 8 } else { 3 };
                        let v = color(1, tile, px, py % 8);
                        if v == 0 { 0 } else { 24 + v as u16 }
                    } else {
                        0
                    };
                    assert_eq!(
                        p.framebuffer()[y * 256 + x],
                        expected,
                        "size={height}, flip={flip}, ({x},{y})"
                    );
                }
            }
        }
    }
}
#[test]
fn first_opaque_sprite_wins_even_when_behind_background() {
    let (mut p, c) = scene(&[[19, 1, 0x20, 40], [19, 2, 1, 40]], 0, 0x1e);
    finish(&mut p, &c);
    for x in 40..48 {
        let bg = color(0, 0, x % 8, 20 % 8);
        let first = color(0, 1, x - 40, 0);
        let second = color(0, 2, x - 40, 0);
        let expected = if first != 0 {
            if bg != 0 {
                bg as u16
            } else {
                16 + first as u16
            }
        } else if second != 0 {
            20 + second as u16
        } else {
            bg as u16
        };
        assert_eq!(p.framebuffer()[20 * 256 + x], expected);
    }
    assert_ne!(p.peek_register(&c, 0x2002) & 0x40, 0);
}
#[test]
fn hit_requires_both_layers_respects_masks_and_never_occurs_at_255() {
    for (x, mask, hit) in [
        (40, 0x1e, true),
        (40, 0x16, false),
        (40, 0x0e, false),
        (0, 0x18, false),
        (0, 0x1a, false),
        (0, 0x1c, false),
        (0, 0x1e, true),
        (255, 0x1e, false),
        (254, 0x1e, true),
    ] {
        let (mut p, c) = scene(&[[19, 1, 0, x]], 0, mask);
        finish(&mut p, &c);
        assert_eq!(
            p.peek_register(&c, 0x2002) & 0x40 != 0,
            hit,
            "x={x}, mask={mask:02x}"
        );
    }
}
#[test]
fn sprite_zero_hit_appears_on_pixel_and_clears_on_prerender() {
    let (mut p, c) = scene(&[[19, 1, 0x20, 40]], 0, 0x1e);
    advance(&mut p, &c, 261, 2);
    advance(&mut p, &c, 20, 41);
    assert_eq!(p.peek_register(&c, 0x2002) & 0x40, 0);
    p.tick(&c); // background transparent at x40
    assert_eq!(p.peek_register(&c, 0x2002) & 0x40, 0);
    p.tick(&c);
    assert_ne!(p.read_register(&c, 0x2002) & 0x40, 0);
    assert_ne!(p.peek_register(&c, 0x2002) & 0x40, 0);
    advance(&mut p, &c, 261, 1);
    assert_ne!(p.peek_register(&c, 0x2002) & 0x40, 0);
    p.tick(&c);
    assert_eq!(p.peek_register(&c, 0x2002) & 0x60, 0);
}
#[test]
fn only_eight_sprites_render_and_overflow_scan_has_false_positives() {
    let mut entries = vec![[19, 1, 0, 40]; 9];
    entries[8][3] = 100;
    let (mut p, c) = scene(&entries, 0, 0x14);
    finish(&mut p, &c);
    assert_ne!(p.peek_register(&c, 0x2002) & 0x20, 0);
    assert_eq!(p.framebuffer()[20 * 256 + 100], 0);
    // Eight Ys match, ninth doesn't, but tenth's tile byte is mistaken for Y.
    entries[8] = [255, 255, 0, 0];
    entries.push([255, 19, 0, 0]);
    let (mut p, c) = scene(&entries, 0, 0x14);
    advance(&mut p, &c, 261, 2);
    advance(&mut p, &c, 19, 257);
    assert_ne!(p.peek_register(&c, 0x2002) & 0x20, 0);
}
#[test]
fn fetches_cover_all_eight_slots_and_oam_writes_are_blocked_during_rendering() {
    let (mut p, mut c) = scene(&[[19, 3, 0, 40]], 8, 0x14);
    advance(&mut p, &c, 19, 257);
    let mut reads = vec![];
    for _ in 257..=320 {
        if let Some(f) = p.tick(&c) {
            reads.push(f.address);
        }
    }
    assert_eq!(reads.len(), 32);
    assert!(reads[..2].iter().all(|a| (0x2000..0x3000).contains(a)));
    assert_eq!(&reads[2..4], &[0x1030, 0x1038]);
    advance(&mut p, &c, 20, 10);
    write(&mut p, &mut c, 0x2003, 0);
    write(&mut p, &mut c, 0x2004, 42);
    write(&mut p, &mut c, 0x2001, 0);
    for _ in 0..2 {
        p.tick(&c);
    }
    write(&mut p, &mut c, 0x2003, 0);
    assert_eq!(p.read_register(&c, 0x2004), 19);
}
#[test]
fn sprite_pipeline_replays_during_evaluation_and_pattern_fetch() {
    for (line, dot) in [(19, 70), (19, 262), (20, 45)] {
        let (mut p, c) = scene(&[[19, 3, 0xc0, 40]], 8, 0x1e);
        advance(&mut p, &c, line, dot);
        let saved = serde_json::to_vec(&p).unwrap();
        let mut restored: Ppu = serde_json::from_slice(&saved).unwrap();
        finish(&mut p, &c);
        finish(&mut restored, &c);
        assert_eq!(
            serde_json::to_vec(&p).unwrap(),
            serde_json::to_vec(&restored).unwrap()
        );
    }
}
