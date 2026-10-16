use nes_core::{cartridge::Cartridge, machine::Machine};
fn machine(battery: bool) -> Machine {
    let mut rom = vec![0; 16 + 16384 + 8192];
    rom[..4].copy_from_slice(b"NES\x1a");
    rom[4] = 1;
    rom[5] = 1;
    rom[6] = if battery { 2 } else { 0 };
    Machine::new(Cartridge::from_ines(&rom).unwrap())
}
#[test]
fn battery_ram_roundtrips_without_exporting_the_rom_or_volatile_state() {
    let mut original = machine(true);
    let bytes: Vec<_> = (0..8192).map(|i| (i % 251) as u8).collect();
    original.restore_battery_ram(&bytes).unwrap();
    assert_eq!(original.battery_ram().unwrap(), bytes);
    let mut fresh = machine(true);
    fresh
        .restore_battery_ram(original.battery_ram().unwrap())
        .unwrap();
    assert_eq!(fresh.peek(0x6105), Some(bytes[0x105]));
    let before = fresh.save_state();
    assert!(fresh.restore_battery_ram(&bytes[..8191]).is_err());
    assert_eq!(fresh.save_state(), before);
    let mut volatile = machine(false);
    assert!(volatile.battery_ram().is_none());
    assert!(volatile.restore_battery_ram(&bytes).is_err());
}
