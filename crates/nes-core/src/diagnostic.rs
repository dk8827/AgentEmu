//! Original NROM program exercising both controllers and mirrored CPU RAM.
//! No commercial code or assets. No PPU/APU interaction yet.
pub fn controller_rom() -> Vec<u8> {
    let code: &[u8] = &[
        0x78, // SEI
        0xd8, // CLD
        0xa2, 0xff, 0x9a, // LDX #$ff; TXS
        0xa9, 1, 0x8d, 0x16, 0x40, // loop: LDA #1; STA $4016
        0xa9, 0, 0x8d, 0x16, 0x40, // LDA #0; STA $4016
        0xa2, 0, // LDX #0
        0xad, 0x16, 0x40, // bit: LDA $4016
        0x29, 1, 0x95, 0, // AND #1; STA $00,X
        0xad, 0x17, 0x40, // LDA $4017
        0x29, 1, 0x95, 0x10, // AND #1; STA $10,X
        0xe8, 0xe0, 8, 0xd0, 0xed, // INX; CPX #8; BNE bit
        0xa9, 0xa5, 0x8d, 0x20, 8, // LDA #$a5; STA $0820 (RAM mirror)
        0xe6, 0x21, // INC $21 (poll counter)
        0x4c, 5, 0x80, // JMP loop
        0xea, 0xea, 0xea, 0xea, 0xea, 0xea, 0xea, 0xea, // padding
    ];
    let mut bytes = vec![0; 16 + 16384];
    bytes[..4].copy_from_slice(b"NES\x1a");
    bytes[4] = 1;
    bytes[16..16 + code.len()].copy_from_slice(code);
    for vector in [0x3ffa, 0x3ffc, 0x3ffe] {
        bytes[16 + vector] = 0;
        bytes[16 + vector + 1] = 0x80;
    }
    bytes
}

/// Original rendering-disabled PPU/OAM DMA/NMI program. No external ROM assets.
pub fn ppu_rom() -> Vec<u8> {
    fn store(code: &mut Vec<u8>, address: u16, value: u8) {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    }
    fn address(code: &mut Vec<u8>, value: u16) {
        store(code, 0x2006, (value >> 8) as u8);
        store(code, 0x2006, value as u8);
    }
    fn read_into(code: &mut Vec<u8>, address: u16, zp: u8) {
        code.extend([0xad, address as u8, (address >> 8) as u8, 0x85, zp]);
    }
    let mut code = vec![0x78, 0xd8, 0xa2, 0xff, 0x9a]; // SEI; CLD; stack
    code.extend([0x2c, 0x02, 0x20]); // discard stale vblank status
    for _ in 0..2 {
        code.extend([0x2c, 0x02, 0x20, 0x10, 0xfb]); // wait for vblank twice
    }
    store(&mut code, 0x2000, 0);
    store(&mut code, 0x2001, 0);
    address(&mut code, 0x0100);
    store(&mut code, 0x2007, 0xab);
    store(&mut code, 0x2007, 0xcd);
    address(&mut code, 0x0100);
    code.extend([0xad, 0x07, 0x20]); // prime read buffer
    read_into(&mut code, 0x2007, 0);
    read_into(&mut code, 0x2007, 1);
    address(&mut code, 0x2000);
    store(&mut code, 0x2007, 0x42);
    address(&mut code, 0x2400); // horizontal mirroring
    code.extend([0xad, 0x07, 0x20]);
    read_into(&mut code, 0x2007, 2);
    address(&mut code, 0x3f10);
    store(&mut code, 0x2007, 0x2a);
    address(&mut code, 0x3f00);
    read_into(&mut code, 0x2007, 3); // unbuffered palette alias
    code.extend([0xa2, 0, 0x8a, 0x9d, 0, 2, 0xe8, 0xd0, 0xf9]); // RAM page 2 = 0..255
    store(&mut code, 0x2003, 0xf0);
    store(&mut code, 0x4014, 2);
    for (addr, zp) in [(0xf0, 4), (0xfe, 5), (0xef, 6)] {
        store(&mut code, 0x2003, addr);
        read_into(&mut code, 0x2004, zp);
    }
    store(&mut code, 0x2000, 0x80); // enable NMI
    store(&mut code, 0x0020, 0xa5); // setup complete
    let loop_addr = 0x8000 + code.len() as u16;
    code.extend([0xe6, 0x21, 0x4c, loop_addr as u8, (loop_addr >> 8) as u8]);
    assert!(code.len() < 0x200);
    let mut bytes = controller_rom();
    bytes[16..16 + 16384].fill(0xea);
    bytes[16..16 + code.len()].copy_from_slice(&code);
    bytes[16 + 0x200..16 + 0x203].copy_from_slice(&[0xe6, 0x10, 0x40]); // NMI: INC $10; RTI
    for (vector, target) in [(0x3ffa, 0x8200u16), (0x3ffc, 0x8000), (0x3ffe, 0x8200)] {
        bytes[16 + vector..16 + vector + 2].copy_from_slice(&target.to_le_bytes());
    }
    bytes
}

/// Original background-only test scene, including all four attribute quadrants.
/// Both nametables use four hand-generated tiles; the second reverses their order.
/// Scroll (5,3) deliberately crosses tile and nametable boundaries.
pub fn background_rom() -> Vec<u8> {
    fn store(code: &mut Vec<u8>, address: u16, value: u8) {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    }
    fn address(code: &mut Vec<u8>, value: u16) {
        store(code, 0x2006, (value >> 8) as u8);
        store(code, 0x2006, value as u8);
    }
    fn branch(code: &mut Vec<u8>, opcode: u8, target: usize) {
        let displacement = i8::try_from(target as isize - (code.len() + 2) as isize).unwrap();
        code.extend([opcode, displacement as u8]);
    }
    let mut code = vec![0x78, 0xd8, 0xa2, 0xff, 0x9a, 0x2c, 2, 0x20];
    for _ in 0..2 {
        code.extend([0x2c, 2, 0x20, 0x10, 0xfb]);
    }
    store(&mut code, 0x2000, 0);
    store(&mut code, 0x2001, 0);
    for (base, flip) in [(0x2000, 0), (0x2400, 3)] {
        address(&mut code, base);
        code.extend([0xa0, 4, 0xa2, 0]); // LDY #4; LDX #0
        let tile_loop = code.len();
        code.extend([0x8a, 0x29, 3, 0x49, flip, 0x8d, 7, 0x20, 0xe8]);
        branch(&mut code, 0xd0, tile_loop);
        code.push(0x88); // DEY
        branch(&mut code, 0xd0, tile_loop);
        address(&mut code, base + 0x3c0);
        code.extend([0xa2, 64, 0xa9, 0xe4]);
        let attribute_loop = code.len();
        code.extend([0x8d, 7, 0x20, 0xca]);
        branch(&mut code, 0xd0, attribute_loop);
    }
    address(&mut code, 0x3f00);
    for value in [
        0x0f, 0x01, 0x21, 0x31, 0x0f, 0x06, 0x16, 0x26, 0x0f, 0x09, 0x19, 0x29, 0x0f, 0x04, 0x14,
        0x24,
    ] {
        store(&mut code, 0x2007, value);
    }
    store(&mut code, 0x2005, 5);
    store(&mut code, 0x2005, 3);
    store(&mut code, 0x2000, 0);
    store(&mut code, 0x2001, 0x0a);
    store(&mut code, 0x0020, 0xa5);
    let end = 0x8000 + code.len() as u16;
    code.extend([0x4c, end as u8, (end >> 8) as u8]);
    let mut rom = controller_rom();
    rom[16..16 + 0x3ffa].fill(0xea);
    rom[16..16 + code.len()].copy_from_slice(&code);
    rom[5] = 1;
    rom[6] = 1; // vertical nametable mirroring
    let mut chr = vec![0; 8192];
    for tile in 0..4 {
        for y in 0..8 {
            for x in 0..8 {
                let color = match tile {
                    0 => 0,
                    1 => ((x + y) % 2) as u8,
                    2 => {
                        if x >= y {
                            3
                        } else {
                            2
                        }
                    }
                    _ => {
                        if x == y || x + y == 7 {
                            1
                        } else {
                            2
                        }
                    }
                };
                chr[tile * 16 + y] |= (color & 1) << (7 - x);
                chr[tile * 16 + y + 8] |= (color >> 1) << (7 - x);
            }
        }
    }
    rom.extend(chr);
    rom
}

/// Original five-channel sound probe; not a recording or commercial sound asset.
pub fn audio_rom() -> Vec<u8> {
    let mut rom = controller_rom();
    let mut code = vec![0x78, 0xd8];
    for (address, value) in [
        (0x4017u16, 0x40u8),
        (0x4015, 0x0f),
        (0x4000, 0xbf),
        (0x4001, 8),
        (0x4002, 253),
        (0x4003, 8),
        (0x4004, 0x7a),
        (0x4005, 8),
        (0x4006, 189),
        (0x4007, 8),
        (0x4008, 0xff),
        (0x400a, 127),
        (0x400b, 8),
        (0x400c, 0x36),
        (0x400e, 7),
        (0x400f, 8),
        (0x4010, 0x4f),
        (0x4011, 64),
        (0x4012, 0),
        (0x4013, 3),
        (0x4015, 0x1f),
    ] {
        code.extend([0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
    }
    let end = 0x8000 + code.len() as u16;
    code.extend([0x4c, end as u8, (end >> 8) as u8]);
    rom[16..16 + code.len()].copy_from_slice(&code);
    rom
}

/// An original two-player playground ROM for testing input, sprites and sound.
/// Move with the D-pad; hold A to play each player's pulse channel.
pub fn playground_rom() -> Vec<u8> {
    use std::collections::BTreeMap;
    struct Program {
        code: Vec<u8>,
        labels: BTreeMap<String, u16>,
        fixups: Vec<(usize, String, bool)>,
    }
    impl Program {
        fn emit(&mut self, bytes: &[u8]) {
            self.code.extend(bytes);
        }
        fn store(&mut self, address: u16, value: u8) {
            self.emit(&[0xa9, value, 0x8d, address as u8, (address >> 8) as u8]);
        }
        fn label(&mut self, name: &str) {
            assert!(
                self.labels
                    .insert(name.into(), 0x8000 + self.code.len() as u16)
                    .is_none()
            );
        }
        fn branch(&mut self, op: u8, label: &str) {
            self.emit(&[op, 0]);
            self.fixups.push((self.code.len() - 1, label.into(), true));
        }
        fn jump(&mut self, label: &str) {
            self.emit(&[0x4c, 0, 0]);
            self.fixups.push((self.code.len() - 2, label.into(), false));
        }
        fn address(&mut self, address: u16) {
            self.store(0x2006, (address >> 8) as u8);
            self.store(0x2006, address as u8);
        }
        fn finish(mut self) -> Vec<u8> {
            for (offset, label, relative) in self.fixups {
                let target = self.labels[&label];
                if relative {
                    self.code[offset] =
                        i8::try_from(target as i32 - (0x8000 + offset as i32 + 1)).unwrap() as u8;
                } else {
                    self.code[offset..offset + 2].copy_from_slice(&target.to_le_bytes());
                }
            }
            self.code
        }
    }
    let mut p = Program {
        code: vec![],
        labels: BTreeMap::new(),
        fixups: vec![],
    };
    p.emit(&[0x78, 0xd8, 0xa2, 0xff, 0x9a]);
    p.store(0x4017, 0x40);
    p.store(0x2000, 0);
    p.store(0x2001, 0);
    p.emit(&[0x2c, 2, 0x20]);
    for name in ["warm1", "warm2"] {
        p.label(name);
        p.emit(&[0x2c, 2, 0x20]);
        p.branch(0x10, name);
    }
    p.address(0x2000);
    p.emit(&[0xa0, 4, 0xa2, 0, 0xa9, 0]);
    p.label("clear");
    p.emit(&[0x8d, 7, 0x20, 0xe8]);
    p.branch(0xd0, "clear");
    p.emit(&[0x88]);
    p.branch(0xd0, "clear");
    p.address(0x2300);
    p.emit(&[0xa2, 32, 0xa9, 2]);
    p.label("floor");
    p.emit(&[0x8d, 7, 0x20, 0xca]);
    p.branch(0xd0, "floor");
    p.address(0x3f00);
    for value in [
        15, 1, 33, 49, 15, 1, 33, 49, 15, 1, 33, 49, 15, 1, 33, 49, 15, 6, 22, 38, 15, 9, 25, 41,
        15, 4, 20, 36, 15, 4, 20, 36,
    ] {
        p.store(0x2007, value);
    }
    p.emit(&[0xa2, 0, 0xa9, 255]);
    p.label("hide");
    p.emit(&[0x9d, 0, 2, 0xe8]);
    p.branch(0xd0, "hide");
    for (address, value) in [(0x10, 64), (0x11, 160), (0x12, 100), (0x13, 100)] {
        p.store(address, value);
    }
    for base in [0x4000, 0x4004] {
        p.store(base, 0xb0);
        p.store(base + 1, 8);
        p.store(base + 2, 180);
    }
    p.store(0x4015, 3);
    p.store(0x4003, 8);
    p.store(0x4007, 8);
    p.store(0x2003, 0);
    p.store(0x4014, 2);
    p.store(0x2005, 0);
    p.store(0x2005, 0);
    p.store(0x2000, 0x80);
    p.store(0x2001, 0x1e);
    p.label("frame");
    p.store(0, 0);
    p.label("wait");
    p.emit(&[0xa5, 0]);
    p.branch(0xf0, "wait");
    p.store(0x4016, 1);
    p.store(0x4016, 0);
    p.emit(&[0xa2, 8]);
    p.label("controllers");
    p.emit(&[
        0xad, 0x16, 0x40, 0x4a, 0x26, 0x20, 0xad, 0x17, 0x40, 0x4a, 0x26, 0x21, 0xca,
    ]);
    p.branch(0xd0, "controllers");
    for player in 0..2u8 {
        for (bit, position, increment) in [
            (1, 0x10 + player, true),
            (2, 0x10 + player, false),
            (4, 0x12 + player, true),
            (8, 0x12 + player, false),
        ] {
            let skip = format!("skip{player}-{bit}");
            p.emit(&[0xa5, 0x20 + player, 0x29, bit]);
            p.branch(0xf0, &skip);
            p.emit(&[if increment { 0xe6 } else { 0xc6 }, position]);
            p.label(&skip);
        }
        // Four 8x8 sprites make each independently controlled 16x16 marker.
        for tile in 0..4u8 {
            let base = 0x200 + player as u16 * 16 + tile as u16 * 4;
            for (position, offset, address) in [
                (0x12 + player, if tile >= 2 { 8 } else { 0 }, base),
                (0x10 + player, (tile & 1) * 8, base + 3),
            ] {
                p.emit(&[0xa5, position, 0x18, 0x69, offset, 0x8d, address as u8, 2]);
            }
            p.store(base + 1, 3);
            p.store(base + 2, player);
        }
        p.emit(&[0xa5, 0x20 + player, 0x29, 0x80]);
        let quiet = format!("quiet{player}");
        let volume = format!("volume{player}");
        p.branch(0xf0, &quiet);
        p.emit(&[0xa9, 0xbf]);
        p.jump(&volume);
        p.label(&quiet);
        p.emit(&[0xa9, 0xb0]);
        p.label(&volume);
        p.emit(&[
            0x8d,
            player * 4,
            0x40,
            0xa5,
            0x10 + player,
            0x8d,
            2 + player * 4,
            0x40,
        ]);
    }
    p.store(0x2003, 0);
    p.store(0x4014, 2);
    p.jump("frame");
    p.label("nmi");
    let nmi = p.labels["nmi"];
    p.emit(&[0xe6, 0, 0x40]);
    let code = p.finish();
    assert!(code.len() < 0x3ffa);
    let mut rom = background_rom();
    rom[16..16 + 0x3ffa].fill(0xea);
    rom[16..16 + code.len()].copy_from_slice(&code);
    for (vector, address) in [(0x3ffa, nmi), (0x3ffc, 0x8000), (0x3ffe, nmi)] {
        rom[16 + vector..16 + vector + 2].copy_from_slice(&address.to_le_bytes());
    }
    rom
}
