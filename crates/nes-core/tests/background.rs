use nes_core::{
    cartridge::Cartridge,
    diagnostic::background_rom,
    machine::Machine,
    ppu::{DOTS_PER_FRAME, Ppu, WRITE_INHIBIT_DOTS},
};

fn write(p: &mut Ppu, c: &mut Cartridge, reg: u16, value: u8) {
    p.write_register(c, reg, value).unwrap();
}
fn address(p: &mut Ppu, c: &mut Cartridge, addr: u16) {
    p.read_register(c, 0x2002);
    write(p, c, 0x2006, (addr >> 8) as u8);
    write(p, c, 0x2006, addr as u8);
}
fn scene(x: u8, y: u8, mask: u8) -> (Ppu, Cartridge) {
    let mut c = Cartridge::from_ines(&background_rom()).unwrap();
    let mut p = Ppu::default();
    for _ in 0..WRITE_INHIBIT_DOTS {
        p.tick(&c);
    }
    for (base, flip) in [(0x2000, 0), (0x2400, 3)] {
        address(&mut p, &mut c, base);
        for tile in 0..960 {
            write(&mut p, &mut c, 0x2007, (tile % 4) as u8 ^ flip);
        }
        for _ in 0..64 {
            write(&mut p, &mut c, 0x2007, 0xe4);
        }
    }
    address(&mut p, &mut c, 0x3f00);
    for color in [15, 1, 33, 49, 15, 6, 22, 38, 15, 9, 25, 41, 15, 4, 20, 36] {
        write(&mut p, &mut c, 0x2007, color);
    }
    write(&mut p, &mut c, 0x2005, x);
    write(&mut p, &mut c, 0x2005, y);
    write(&mut p, &mut c, 0x2000, 0);
    write(&mut p, &mut c, 0x2001, mask);
    (p, c)
}
fn advance_to(p: &mut Ppu, c: &Cartridge, scanline: u16, dot: u16) {
    for _ in 0..=DOTS_PER_FRAME {
        if (p.scanline(), p.dot()) == (scanline, dot) {
            return;
        }
        p.tick(c);
    }
    panic!("unreachable coordinate");
}
fn complete_scene(p: &mut Ppu, c: &Cartridge) {
    let target = p.completed_frames() + 2;
    while p.completed_frames() < target {
        p.tick(c);
    }
}
// Geometric oracle: no PPU address, fetch timing, or shift-register logic.
fn expected_pixel(x: usize, y: usize, scroll_x: usize, scroll_y: usize) -> u16 {
    let wx = (x + scroll_x) % 512;
    let wy = (y + scroll_y) % 240;
    let tile = ((wx / 8) % 4) ^ if wx >= 256 { 3 } else { 0 };
    let (px, py) = (wx % 8, wy % 8);
    let color = match tile {
        0 => 0,
        1 => (px + py) % 2,
        2 => {
            if px >= py {
                3
            } else {
                2
            }
        }
        _ => {
            if px == py || px + py == 7 {
                1
            } else {
                2
            }
        }
    };
    if color == 0 {
        return 15;
    }
    let palette = ((wx / 16) % 2) + 2 * ((wy / 16) % 2);
    [
        [15, 1, 33, 49],
        [15, 6, 22, 38],
        [15, 9, 25, 41],
        [15, 4, 20, 36],
    ][palette][color]
}

#[test]
fn original_rom_renders_every_pixel_with_scroll_and_nametable_wrap() {
    let mut m = Machine::new(Cartridge::from_ines(&background_rom()).unwrap());
    for _ in 0..8 {
        m.run_frame().unwrap();
    }
    assert_eq!(m.peek(0x20), Some(0xa5));
    assert_eq!(m.ppu().completed_frames(), 8);
    for y in 0..240 {
        for x in 0..256 {
            assert_eq!(
                m.ppu().framebuffer()[y * 256 + x],
                expected_pixel(x, y, 5, 3),
                "({x},{y})"
            );
        }
    }
    let actual: serde_json::Value = serde_json::from_str(&m.diagnostic_report()).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/diagnostic/background-native.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn fine_scroll_all_eight_offsets_attribute_quadrants_and_left_clipping() {
    for fine in 0..8 {
        let (mut p, c) = scene(248 + fine, 235, 0x08);
        complete_scene(&mut p, &c);
        for y in 0..240 {
            for x in 0..256 {
                let expected = if x < 8 {
                    15
                } else {
                    expected_pixel(x, y, (248 + fine) as usize, 235)
                };
                assert_eq!(
                    p.framebuffer()[y * 256 + x],
                    expected,
                    "scroll={fine}, ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn fetch_schedule_prefetches_two_tiles_before_first_visible_pixel() {
    let (mut p, c) = scene(5, 3, 0x0a);
    advance_to(&mut p, &c, 261, 321);
    let mut reads = vec![];
    for _ in 321..=336 {
        if let Some(fetch) = p.tick(&c) {
            reads.push((fetch.dot % 341, fetch.address));
        }
    }
    assert_eq!(
        reads,
        [
            (322, 0x2000),
            (324, 0x23c0),
            (326, 3),
            (328, 11),
            (330, 0x2001),
            (332, 0x23c0),
            (334, 19),
            (336, 27)
        ]
    );
    // Peeking CHR must not disturb the fetch latches or clocks.
    let before = serde_json::to_vec(&p).unwrap();
    assert_eq!(p.peek_memory(&c, 3), 0);
    assert_eq!(serde_json::to_vec(&p).unwrap(), before);
}

#[test]
fn rendering_shortens_only_enabled_odd_frames() {
    let (mut p, mut c) = scene(0, 0, 0x0a);
    advance_to(&mut p, &c, 0, 0);
    for expected in [89341, 89342, 89341] {
        let frame = p.frame();
        let start = p.total_dots();
        while p.frame() == frame {
            p.tick(&c);
        }
        assert_eq!(p.total_dots() - start, expected);
    }
    write(&mut p, &mut c, 0x2001, 0);
    for _ in 0..2 {
        let start = p.total_dots();
        let frame = p.frame();
        while p.frame() == frame {
            p.tick(&c);
        }
        assert_eq!(p.total_dots() - start, DOTS_PER_FRAME);
    }
}

#[test]
fn coarse_y_29_toggles_nametable_but_31_wraps_without_toggling() {
    for (scroll, expected_y) in [(239, 0x0800), (255, 0)] {
        let (mut p, c) = scene(0, scroll, 0x0a);
        advance_to(&mut p, &c, 0, 257); // line zero's increment at 256 has executed
        let state = serde_json::to_value(p).unwrap();
        assert_eq!(state["v"].as_u64().unwrap() & 0x7be0, expected_y);
    }
}

#[test]
fn grayscale_emphasis_pattern_table_selection_and_forced_blank_palette() {
    let (mut p, mut c) = scene(5, 3, 0xeb); // grayscale and all emphasis bits
    complete_scene(&mut p, &c);
    for y in 0..240 {
        for x in 0..256 {
            assert_eq!(
                p.framebuffer()[y * 256 + x],
                (expected_pixel(x, y, 5, 3) & 0x30) | 0x1c0
            );
        }
    }
    write(&mut p, &mut c, 0x2000, 0x10); // second pattern table is all transparent
    complete_scene(&mut p, &c);
    assert!(p.framebuffer().iter().all(|pixel| *pixel == 0x1c0));
    write(&mut p, &mut c, 0x2001, 0);
    address(&mut p, &mut c, 0x3f03);
    complete_scene(&mut p, &c);
    assert!(p.framebuffer().iter().all(|pixel| *pixel == 49));
}

#[test]
fn partial_frame_snapshot_preserves_pipeline_completed_frame_and_future_pixels() {
    let mut m = Machine::new(Cartridge::from_ines(&background_rom()).unwrap());
    for _ in 0..6 {
        m.run_frame().unwrap();
    }
    // Stop within a tile fetch, not merely at a frame boundary.
    while m.ppu().scanline() != 57 || !(3..=5).contains(&m.ppu().dot()) {
        m.step().unwrap();
    }
    let front = m.ppu().framebuffer().to_vec();
    let saved = m.save_state();
    m.run_instructions(17).unwrap();
    assert_eq!(m.ppu().framebuffer(), front);
    m.run_frame().unwrap();
    let expected = m.save_state();
    m.load_state(&saved).unwrap();
    assert_eq!(m.ppu().framebuffer(), front);
    m.run_instructions(17).unwrap();
    m.run_frame().unwrap();
    assert_eq!(m.save_state(), expected);
    let before = m.save_state();
    for (key, value) in [
        ("front", serde_json::json!([])),
        ("skipped_dots", serde_json::json!(9999)),
        ("completed_frames", serde_json::json!(9999)),
        ("dot", serde_json::json!(341)),
    ] {
        let mut bad: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        bad["ppu"][key] = value;
        assert!(
            m.load_state(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{key}"
        );
        assert_eq!(m.save_state(), before);
    }
    let mut bad: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    bad["ppu"]["front"][0] = serde_json::json!(512);
    assert!(m.load_state(&serde_json::to_vec(&bad).unwrap()).is_err());
    assert_eq!(m.save_state(), before);
}

#[test]
fn ppudata_during_rendering_increments_scroll_axes_regardless_of_stride_bit() {
    let (mut p, mut c) = scene(0, 0, 0x0a);
    advance_to(&mut p, &c, 0, 9);
    for ctrl in [0, 4] {
        write(&mut p, &mut c, 0x2000, ctrl);
        address(&mut p, &mut c, 0x03bf); // coarse X=31, coarse Y=29, fine Y=0
        p.read_register(&c, 0x2007);
        assert_eq!(serde_json::to_value(&p).unwrap()["v"], 0x17a0);
        address(&mut p, &mut c, 0x03bf);
        write(&mut p, &mut c, 0x2007, 0);
        assert_eq!(serde_json::to_value(&p).unwrap()["v"], 0x17a0);
    }
}
