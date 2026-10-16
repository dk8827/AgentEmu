use nes_core::{cartridge::Cartridge, diagnostic::playground_rom, machine::Machine};
#[test]
fn original_playground_has_two_independent_players_and_sound() {
    let mut m = Machine::new(Cartridge::from_ines(&playground_rom()).unwrap());
    for _ in 0..10 {
        m.run_frame().unwrap();
    }
    let x1 = m.peek(0x10).unwrap();
    let x2 = m.peek(0x11).unwrap();
    m.set_controllers([0x81, 0x40]);
    for _ in 0..10 {
        m.run_frame().unwrap();
    }
    assert_eq!(m.peek(0x10), Some(x1 + 10));
    assert_eq!(m.peek(0x11), Some(x2 - 10));
    assert!(m.ppu().framebuffer().contains(&22));
    assert!(m.ppu().framebuffer().contains(&25));
    m.take_audio();
    let saved = m.save_state();
    m.run_frame().unwrap();
    let audio = m.take_audio();
    assert!(audio.iter().any(|v| v.abs() > 0.01));
    let next = m.save_state();
    m.load_state(&saved).unwrap();
    m.run_frame().unwrap();
    assert_eq!(m.save_state(), next);
    assert_eq!(m.take_audio(), audio);
}
