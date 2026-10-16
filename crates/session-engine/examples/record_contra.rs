//! Render a local quality-evaluation replay with the game's original audio.
//! ROMs, decoded frames, audio and video remain outside the repository.
use session_engine::{
    episode::Episode,
    runner::{Input, advance},
};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    process::{Command, Stdio},
};

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: record_contra ROM REPLAY_WITH_RESULT_JSON NEW_MP4".into());
    }
    let replay: serde_json::Value = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let row = &replay["result"];
    let inputs: Vec<[u8; 2]> = serde_json::from_value(replay["inputs"].clone())?;
    if inputs.is_empty()
        || inputs.len() > 108000
        || row["frames"].as_u64() != Some(inputs.len() as u64)
    {
        return Err("invalid replay length".into());
    }
    let bytes: Vec<u8> = inputs.iter().flatten().copied().collect();
    if format!("{:x}", Sha256::digest(&bytes))
        != row["input_sha256"].as_str().ok_or("missing input hash")?
    {
        return Err("replay input digest mismatch".into());
    }
    let output = Path::new(&args[2]);
    if output.exists() {
        return Err("output must be new".into());
    }
    let silent = output.with_extension("silent.mp4");
    let pcm = output.with_extension("audio.f32");
    let mut audio = BufWriter::new(OpenOptions::new().write(true).create_new(true).open(&pcm)?);
    let mut encoder = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-n",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-s",
            "256x240",
            "-framerate",
            "60.0988",
            "-i",
            "pipe:0",
            "-an",
            "-vf",
            "scale=768:720:flags=neighbor",
            "-c:v",
            "libx264",
            "-threads",
            "2",
            "-preset",
            "fast",
            "-crf",
            "20",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&silent)
        .stdin(Stdio::piped())
        .spawn()?;
    let mut video = BufWriter::new(encoder.stdin.take().ok_or("missing encoder input")?);
    let palette = include_bytes!("../../nes-video/palettes/2c02.pal");
    let mut core = Episode::new(&std::fs::read(&args[0])?, 108000)?;
    core.reset_timing(row["warmup"].as_u64().ok_or("missing warmup")?.try_into()?)?;
    let mut rgb = vec![0; 256 * 240 * 3];
    let mut samples = 0u64;
    for buttons in inputs {
        let frame = advance(&mut core.machine, Input::Replay(buttons))?;
        for sample in frame.audio {
            audio.write_all(&sample.to_le_bytes())?;
            samples += 1;
        }
        for (pixel, code) in rgb
            .chunks_exact_mut(3)
            .zip(core.machine.ppu().framebuffer())
        {
            let offset = usize::from(code & 511) * 3;
            pixel.copy_from_slice(&palette[offset..offset + 3]);
        }
        video.write_all(&rgb)?;
    }
    video.flush()?;
    drop(video);
    audio.flush()?;
    drop(audio);
    if !encoder.wait()?.success() {
        return Err("video encoder failed".into());
    }
    if format!("{:x}", Sha256::digest(core.machine.save_state()))
        != row["final_state_sha256"]
            .as_str()
            .ok_or("missing state hash")?
    {
        return Err("recorded replay differs from evaluated final state".into());
    }
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-n", "-i"])
        .arg(&silent)
        .args(["-f", "f32le", "-ar", "48000", "-ac", "1", "-i"])
        .arg(&pcm)
        .args([
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-movflags",
            "+faststart",
        ])
        .arg(output)
        .status()?;
    if !status.success() {
        return Err("audio mux failed".into());
    }
    std::fs::remove_file(silent)?;
    std::fs::remove_file(pcm)?;
    let mut metadata = File::create(output.with_extension("verification.json"))?;
    writeln!(
        metadata,
        "{}",
        serde_json::json!({"input_sha256":row["input_sha256"],"final_state_sha256":row["final_state_sha256"],"audio_samples":samples,"frames":row["frames"],"source":"exact replay; original emulator audio"})
    )?;
    Ok(())
}
