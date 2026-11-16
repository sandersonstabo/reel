use std::{
    ffi::c_void,
    fs, io,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use windows::{
    Win32::{
        Foundation::{E_FAIL, HWND},
        Media::MediaFoundation::{MF_VERSION, MFSTARTUP_FULL, MFShutdown, MFStartup},
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx},
        UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
    },
    core::{Error, Result},
};

use super::{audio, capture::{Capture, Stats}, clock, gpu::Gpu, writer::Writer};

fn record(window: HWND, output: &Path) -> Result<u64> {
    let gpu = Gpu::create()?;
    let writer = Writer::create(&gpu, output)?;
    eprintln!("recorder: video encoder chain: {}", writer.video_chain());
    let writer = Arc::new(Mutex::new(writer));
    let origin = clock::now();
    let (stop, stopped) = mpsc::channel();
    let stats = Arc::new(Stats::default());
    let capture = Capture::start(&gpu, window, writer.clone(), origin, stop.clone(), stats.clone())?;
    let running = Arc::new(AtomicBool::new(true));
    let audio = {
        let writer = writer.clone();
        let running = running.clone();
        thread::spawn(move || audio::pump(writer, origin, running))
    };
    thread::spawn(move || {
        let _ = io::copy(&mut io::stdin(), &mut io::sink());
        let _ = stop.send(());
    });
    let _ = stopped.recv();
    capture.stop();
    running.store(false, Ordering::Relaxed);
    let _ = audio.join();
    eprintln!("recorder: frames {}", stats.summary());
    let finished = writer.lock().map_err(|_| Error::from(E_FAIL))?.finish();
    finished
}

pub fn main(window: isize, output: &Path) -> i32 {
    let started = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .and_then(|_| unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) });
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let outcome = started.and_then(|_| record(HWND(window as *mut c_void), output));
    let _ = unsafe { MFShutdown() };
    match outcome {
        Ok(0) => {
            let _ = fs::remove_file(output);
            eprintln!("recorder: no frames captured, removed {}", output.display());
            0
        }
        Ok(frames) => {
            eprintln!("recorder: wrote {frames} frames to {}", output.display());
            0
        }
        Err(error) => {
            eprintln!("recorder: failed for {}: {error}", output.display());
            1
        }
    }
}
