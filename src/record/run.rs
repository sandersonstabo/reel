use std::{
    ffi::c_void,
    fs,
    io::{self, BufRead},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU16, Ordering},
        mpsc,
    },
    thread,
};

use windows::{
    Win32::{
        Foundation::{E_FAIL, HWND},
        Media::MediaFoundation::{MF_VERSION, MFSTARTUP_FULL, MFShutdown, MFStartup},
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx},
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
            WindowsAndMessaging::GetWindowThreadProcessId,
        },
    },
    core::{Error, Result},
};

use super::{
    CHANNELS, SAMPLE_RATE, TICKS_PER_SECOND, audio, level,
    capture::{self, Capture, Stats, Wiring},
    clock,
    encode::video::{self, VideoEncoder},
    gpu::Gpu,
    plan::{self, KEYFRAME_SECONDS, Plan},
    ring::Ring,
};
use crate::{
    mux::clip::{self, Format},
    settings::{Settings, store},
};

const FRAME_QUEUE: usize = 3;

enum Command {
    Save(PathBuf),
    Stop,
}

fn parse(line: &str) -> Option<Command> {
    line.strip_prefix("save ")
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| Command::Save(PathBuf::from(path)))
}

struct Buffer {
    ring: Arc<Mutex<Ring>>,
    header: Arc<Mutex<Option<Vec<u8>>>>,
    plan: Plan,
    settings: Settings,
}

impl Buffer {
    fn save(&self, path: &Path) -> std::result::Result<(f64, usize), String> {
        let length = i64::from(self.settings.clip_length.seconds()) * TICKS_PER_SECOND;
        let clip = self
            .ring
            .lock()
            .map_err(|_| "buffer lock poisoned".to_string())?
            .clip(length)
            .ok_or_else(|| "nothing buffered yet".to_string())?;
        let format = Format {
            codec: self.plan.codec,
            fps: self.plan.fps,
            width: self.plan.resolution.width,
            height: self.plan.resolution.height,
            rate: SAMPLE_RATE,
            channels: CHANNELS,
            audio_bitrate: self.plan.audio_bitrate,
            sequence_header: self.header.lock().ok().and_then(|header| header.clone()),
        };
        let bytes = clip::build(&clip, &format).map_err(|error| error.to_string())?;
        if let Some(directory) = path.parent() {
            fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        }
        fs::write(path, &bytes).map_err(|error| error.to_string())?;
        let seconds = clip
            .video
            .last()
            .map_or(0.0, |last| (last.time + last.duration) as f64 / TICKS_PER_SECOND as f64);
        Ok((seconds, bytes.len()))
    }
}

fn record(window: HWND) -> Result<()> {
    let settings = store::load();
    let mut process = 0u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
    let gpu = Gpu::create()?;
    let item = capture::item(window)?;
    let size = item.Size()?;
    let plan = plan::plan(plan::native(size.Width.max(2) as u32, size.Height.max(2) as u32), &settings);
    let (encoder, rejected) = VideoEncoder::create(&gpu.manager, &plan)?;
    eprintln!(
        "recorder: buffering {} s of {} {} at {:.1} Mbps with {}{}",
        settings.clip_length.seconds(),
        plan.codec.key(),
        plan::label(&plan),
        f64::from(plan.video_bitrate) / 1e6,
        encoder.name,
        match rejected.is_empty() {
            true => String::new(),
            false => format!(" ({} encoder settings rejected)", rejected.len()),
        }
    );
    let span = i64::from(settings.clip_length.seconds() + KEYFRAME_SECONDS) * TICKS_PER_SECOND;
    let buffer = Buffer {
        ring: Arc::new(Mutex::new(Ring::new(span))),
        header: Arc::new(Mutex::new(None)),
        plan,
        settings,
    };
    let running = Arc::new(AtomicBool::new(true));
    let origin = clock::now();
    let (frames, frame_queue) = mpsc::sync_channel(FRAME_QUEUE);
    let (commands, inbox) = mpsc::channel();
    let (closed, closures) = mpsc::channel();
    let stats = Arc::new(Stats::default());
    {
        let ring = buffer.ring.clone();
        let header = buffer.header.clone();
        let running = running.clone();
        thread::spawn(move || {
            if let Err(error) = encoder.pump(frame_queue, ring, header, running) {
                eprintln!("recorder: video encoder stopped: {error}");
            }
        });
    }
    let capture = Capture::start(
        &gpu,
        item,
        &plan,
        Wiring {
            allocator: video::allocator(&gpu.manager, &plan)?,
            frames,
            closed,
            stats: stats.clone(),
            origin,
        },
    )?;
    let peak = Arc::new(AtomicU16::new(0));
    let audio = {
        let ring = buffer.ring.clone();
        let peak = peak.clone();
        let running = running.clone();
        thread::spawn(move || audio::pump(process, ring, peak, plan.audio_bitrate, origin, running))
    };
    {
        let commands = commands.clone();
        thread::spawn(move || {
            io::stdin()
                .lock()
                .lines()
                .map_while(std::result::Result::ok)
                .filter_map(|line| parse(&line))
                .for_each(|command| {
                    let _ = commands.send(command);
                });
            let _ = commands.send(Command::Stop);
        });
    }
    thread::spawn(move || {
        if closures.recv().is_ok() {
            let _ = commands.send(Command::Stop);
        }
    });
    for command in inbox {
        match command {
            Command::Save(path) => match buffer.save(&path) {
                Ok((seconds, bytes)) => eprintln!(
                    "recorder: saved {seconds:.1} s clip ({:.1} MB) to {}",
                    bytes as f64 / 1e6,
                    path.display()
                ),
                Err(error) => eprintln!("recorder: clip failed: {error}"),
            },
            Command::Stop => break,
        }
    }
    capture.stop();
    running.store(false, Ordering::Relaxed);
    let _ = audio.join();
    eprintln!("recorder: frames {}", stats.summary());
    eprintln!(
        "recorder: game audio from process {process} was {}",
        level::describe(peak.load(Ordering::Relaxed))
    );
    buffer.ring.lock().map(|_| ()).map_err(|_| Error::from(E_FAIL))
}

pub fn main(window: isize) -> i32 {
    let started = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .and_then(|_| unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) });
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let outcome = started.and_then(|_| record(HWND(window as *mut c_void)));
    let _ = unsafe { MFShutdown() };
    match outcome {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("recorder: failed: {error}");
            1
        }
    }
}
