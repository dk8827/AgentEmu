use nes_core::{cartridge::Cartridge, diagnostic::background_rom};
#[test]
fn disk_dude_signature_uses_legacy_fields_and_preserves_file_identity() {
    let mut rom = background_rom();
    let clean = Cartridge::from_ines(&rom).unwrap();
    rom[7..16].copy_from_slice(b"DiskDude!");
    let old = Cartridge::from_ines(&rom).unwrap();
    assert_eq!(old.mapper_id(), clean.mapper_id());
    assert_eq!(old.cpu_read(0x8000), clean.cpu_read(0x8000));
    assert_ne!(old.digest(), clean.digest());
    rom[7] = 8;
    assert!(Cartridge::from_ines(&rom).is_err());
}
