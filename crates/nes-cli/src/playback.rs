//! Local compatibility harness. ROM and image/audio outputs are never CI fixtures.
use nes_core::{cartridge::Cartridge, machine::Machine};
use serde_json::{Value, json};
use std::{error::Error, path::Path};

pub fn run(rom: &str, script: &str, output: &str) -> Result<(), Box<dyn Error>> {
    let script: Value = serde_json::from_slice(&std::fs::read(script)?)?;
    let segments = script["segments"].as_array().ok_or("missing segments")?;
    let mut cart = Cartridge::from_ines(&std::fs::read(rom)?)?;
    let mapper = cart.mapper_id();
    if script["mmc3_alternate"].as_bool().unwrap_or(false) {
        cart.set_mmc3_alternate(true)?;
    }
    let mut m = Machine::new(cart);
    let output = Path::new(output);
    std::fs::create_dir_all(output)?;
    let mut frames = 0;
    let mut checkpoints = Vec::new();
    for (index, segment) in segments.iter().enumerate() {
        let count = segment["frames"].as_u64().ok_or("missing frame count")?;
        if count == 0 || count > 216_000 - frames {
            return Err("frame budget is 1..216000".into());
        }
        let p = segment["players"].as_array().ok_or("missing two players")?;
        if p.len() != 2 {
            return Err("expected two controller masks".into());
        }
        let mask = |i: usize| -> Result<u8, Box<dyn Error>> {
            Ok(p[i]
                .as_u64()
                .filter(|n| *n <= 255)
                .ok_or("invalid controller mask")? as u8)
        };
        m.set_controllers([mask(0)?, mask(1)?]);
        if let Some(z) = segment["zapper"].as_array() {
            if z.len() != 3 {
                return Err("expected [x,y,trigger] for zapper".into());
            }
            let x = i16::try_from(z[0].as_i64().ok_or("invalid zapper x")?)?;
            let y = i16::try_from(z[1].as_i64().ok_or("invalid zapper y")?)?;
            m.set_zapper(x, y, z[2].as_bool().ok_or("invalid zapper trigger")?);
        }
        let mut audio = Vec::new();
        for _ in 0..count {
            m.run_frame()?;
            audio.extend(m.take_audio());
        }
        frames += count;
        let snapshot = m.save_state();
        m.run_frame()?;
        let next_state = m.save_state();
        let next_audio = m.take_audio();
        m.load_state(&snapshot)?;
        m.run_frame()?;
        if next_state != m.save_state() || next_audio != m.take_audio() {
            return Err("snapshot replay mismatch".into());
        }
        m.load_state(&snapshot)?;
        let codes: Vec<u8> = m
            .ppu()
            .framebuffer()
            .iter()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        std::fs::write(output.join(format!("{index:02}.pixels")), codes)?;
        let pcm: Vec<u8> = audio.iter().flat_map(|s| s.to_le_bytes()).collect();
        std::fs::write(output.join(format!("{index:02}.f32")), pcm)?;
        let report: Value = serde_json::from_str(&m.diagnostic_report())?;
        let rms =
            (audio.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / audio.len() as f64).sqrt();
        checkpoints.push(json!({"frames":frames,"report":report,"audio_rms":rms,"audio_samples":audio.len(),"replay":"passed"}));
    }
    let report = json!({"rom_sha256":m.cartridge_hash(),"mapper":mapper,"script":script,"checkpoints":checkpoints});
    std::fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "{}",
        json!({"frames":frames,"mapper":mapper,"rom_sha256":m.cartridge_hash(),"replay":"passed"})
    );
    Ok(())
}
