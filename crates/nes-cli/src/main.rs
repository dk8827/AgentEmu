use nes_core::{
    cartridge::Cartridge,
    diagnostic::{audio_rom, background_rom, controller_rom, ppu_rom},
    machine::Machine,
};
use std::{error::Error, io::Write};
mod playback;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("playback") if args.len() == 4 => playback::run(&args[1], &args[2], &args[3])?,
        Some("diagnose") if args.len() == 1 => {
            let mut machine = Machine::new(Cartridge::from_ines(&controller_rom())?);
            machine.set_controllers([0xa5, 0x5a]);
            machine.run_instructions(1000)?;
            println!("{}", machine.diagnostic_report());
        }
        Some("diagnose-ppu") if args.len() == 1 => {
            let mut machine = Machine::new(Cartridge::from_ines(&ppu_rom())?);
            machine.run_instructions(50_000)?;
            println!("{}", machine.diagnostic_report());
        }
        Some("diagnose-background") if args.len() == 1 => {
            let mut machine = Machine::new(Cartridge::from_ines(&background_rom())?);
            for _ in 0..8 {
                machine.run_frame()?;
            }
            println!("{}", machine.diagnostic_report());
        }
        Some("diagnose-audio") if args.len() == 2 => {
            let mut m = Machine::new(Cartridge::from_ines(&audio_rom())?);
            for _ in 0..8 {
                m.run_frame()?;
            }
            let samples = m.take_audio();
            let mut bytes = Vec::with_capacity(samples.len() * 4);
            for sample in samples {
                bytes.extend(sample.to_le_bytes());
            }
            std::fs::write(&args[1], bytes)?;
            println!("{}", m.diagnostic_report());
        }
        Some("write-diagnostic") if args.len() == 2 => {
            // Never overwrite a user's existing ROM.
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&args[1])?;
            file.write_all(&controller_rom())?;
        }
        Some("test-rom") if args.len() == 3 || (args.len() == 4 && args[3] == "--mmc3-alt") => {
            let limit: u32 = args[2].parse()?;
            if !(1..=36000).contains(&limit) {
                return Err("frame limit must be 1..36000".into());
            }
            let rom = std::fs::read(&args[1])?;
            let mut cartridge = Cartridge::from_ines(&rom)?;
            if args.len() == 4 {
                cartridge.set_mmc3_alternate(true)?;
            }
            let mut m = Machine::new(cartridge);
            for frame in 1..=limit {
                m.run_frame()?;
                m.take_audio();
                let magic = (1..=3)
                    .map(|i| m.peek(0x6000 + i).unwrap_or(0))
                    .collect::<Vec<_>>();
                if magic == [0xde, 0xb0, 0x61] {
                    let code = m.peek(0x6000).unwrap();
                    if code < 128 {
                        let message = (0x6004..0x6800)
                            .map(|a| m.peek(a).unwrap_or(0))
                            .take_while(|b| *b != 0)
                            .collect::<Vec<_>>();
                        println!("{}", String::from_utf8_lossy(&message));
                        println!("result={code} frames={frame} cycles={}", m.cpu.cycles);
                        if code != 0 {
                            return Err("test ROM failed".into());
                        }
                        return Ok(());
                    }
                } else {
                    let text = (0x2000..0x23c0)
                        .map(|a| m.peek_ppu(a) as char)
                        .collect::<String>()
                        .to_ascii_uppercase();
                    if text.contains("PASSED") {
                        println!("PASSED frames={frame} cycles={}", m.cpu.cycles);
                        return Ok(());
                    }
                    if text.contains("FAILED") {
                        println!("{}", text.trim());
                        return Err("test ROM failed".into());
                    }
                }
            }
            let message = (0x6004..0x6800)
                .map(|a| m.peek(a).unwrap_or(0))
                .take_while(|b| *b != 0)
                .collect::<Vec<_>>();
            println!("{}", String::from_utf8_lossy(&message));
            println!(
                "timeout pc={:04x} cycles={} status={:?}",
                m.cpu.pc,
                m.cpu.cycles,
                m.peek(0x6000)
            );
            return Err("test ROM timed out".into());
        }
        Some("trace") if args.len() == 3 => {
            let steps: usize = args[2].parse()?;
            if steps > 10_000_000 {
                return Err("trace limit is 10 million instructions".into());
            }
            let rom = std::fs::read(&args[1])?;
            let mut machine = Machine::new(Cartridge::from_ines(&rom)?);
            let stdout = std::io::stdout();
            let mut output = std::io::BufWriter::new(stdout.lock());
            for _ in 0..steps {
                let cpu = machine.cpu;
                writeln!(
                    output,
                    "{:04X} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{}",
                    cpu.pc, cpu.a, cpu.x, cpu.y, cpu.p, cpu.sp, cpu.cycles
                )?;
                machine.step()?;
            }
        }
        _ => {
            return Err(
                "usage: nes-cli diagnose | diagnose-audio PCM_PATH | diagnose-ppu | diagnose-background | write-diagnostic NEW_PATH | test-rom ROM_PATH FRAMES | trace ROM_PATH INSTRUCTIONS | playback ROM_PATH SCRIPT_JSON OUTPUT_DIRECTORY"
                    .into(),
            );
        }
    }
    Ok(())
}
