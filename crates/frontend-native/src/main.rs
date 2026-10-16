use player_ui::LaunchOptions;
use std::{error::Error, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let mut options = LaunchOptions::default();
    let mut args = std::env::args().skip(1);
    let mut path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ai-games" => options.ai_games = true,
            "--mute" => options.muted = true,
            "--autoplay" => options.autoplay = true,
            "--mmc3-alt" => options.alternate = true,
            "--frames" => {
                options.exit_frames = Some(args.next().ok_or("--frames needs a count")?.parse()?)
            }
            "--screenshot" => {
                options.screenshot = Some(args.next().ok_or("--screenshot needs a PNG path")?)
            }
            "--size" => {
                let size = args.next().ok_or("--size needs WIDTHxHEIGHT")?;
                let (w, h) = size.split_once('x').ok_or("--size needs WIDTHxHEIGHT")?;
                options.size = Some([w.parse()?, h.parse()?]);
            }
            "--state" => {
                options.state = Some(std::fs::read(args.next().ok_or("--state needs a file")?)?)
            }
            "--contra-start" => options.start_task = Some("contra-jungle".into()),
            "--snow-start" => options.start_task = Some("snowbros-floor1".into()),
            "--task-start" => {
                options.start_task = Some(args.next().ok_or("--task-start needs a task name")?)
            }
            "--agent1" | "--agent2" => {
                let p = PathBuf::from(args.next().ok_or("agent option needs a bundle directory")?);
                if std::fs::metadata(p.join("manifest.json"))?.len() > 16384
                    || std::fs::metadata(p.join("weights.safetensors"))?.len() > 131072
                {
                    return Err("agent bundle exceeds its size limit".into());
                }
                options.agents[usize::from(arg == "--agent2")] = Some((
                    std::fs::read_to_string(p.join("manifest.json"))?,
                    std::fs::read(p.join("weights.safetensors"))?,
                ));
            }
            "--help" => {
                println!(
                    "AgentEmu — NES emulator with shared human/AI control\nagentemu [ROM.nes] [--autoplay] [--ai-games] [--mute] [--state FILE] [--agent1 DIRECTORY] [--agent2 DIRECTORY] [--contra-start | --snow-start | --task-start TASK] [--mmc3-alt] [--frames N] [--screenshot PNG] [--size WIDTHxHEIGHT]\nOpen without arguments for the game picker. Escape pauses and opens the menu; F11 toggles fullscreen. All controls and agents are configurable in the interface."
                );
                return Ok(());
            }
            _ if !arg.starts_with('-') && path.is_none() => path = Some(PathBuf::from(arg)),
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if let Some(p) = path {
        options.rom = Some((
            p.file_name().unwrap_or_default().to_string_lossy().into(),
            std::fs::read(p)?,
        ));
    }
    player_ui::run(options).map_err(|e| e.to_string().into())
}
