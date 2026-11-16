use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use windows::{
    Win32::{
        Foundation::E_FAIL,
        Media::Audio::{
            AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
            AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
            WAVE_FORMAT_PCM, WAVEFORMATEX, eConsole, eRender,
        },
        System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx},
    },
    core::{Error, Result},
};

use super::{
    CHANNELS, SAMPLE_RATE, TICKS_PER_SECOND,
    clock,
    timeline::{Alignment, Timeline},
    writer::Writer,
};

const BYTES_PER_FRAME: usize = CHANNELS as usize * 2;
const BUFFER_TICKS: i64 = TICKS_PER_SECOND / 5;
const TOLERANCE_TICKS: i64 = TICKS_PER_SECOND / 50;
const FILL_LAG_TICKS: i64 = TICKS_PER_SECOND / 10;
const POLL: Duration = Duration::from_millis(10);
const REOPEN: Duration = Duration::from_millis(500);
const MAX_CHUNK_FRAMES: u64 = SAMPLE_RATE as u64;

struct Loopback {
    client: IAudioClient,
    capture: IAudioCaptureClient,
}

impl Loopback {
    fn open() -> Result<Self> {
        let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole)? };
        let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None)? };
        let format = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: CHANNELS,
            nSamplesPerSec: SAMPLE_RATE,
            nAvgBytesPerSec: SAMPLE_RATE * BYTES_PER_FRAME as u32,
            nBlockAlign: BYTES_PER_FRAME as u16,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK
                    | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                    | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                BUFFER_TICKS,
                0,
                &format,
                None,
            )?
        };
        let capture: IAudioCaptureClient = unsafe { client.GetService()? };
        unsafe { client.Start()? };
        Ok(Self { client, capture })
    }
}

impl Drop for Loopback {
    fn drop(&mut self) {
        let _ = unsafe { self.client.Stop() };
    }
}

struct Sink {
    writer: Arc<Mutex<Writer>>,
    timeline: Timeline,
    origin: i64,
}

impl Sink {
    fn write(&mut self, pcm: &[u8]) -> Result<()> {
        let frames = (pcm.len() / BYTES_PER_FRAME) as u64;
        match frames {
            0 => Ok(()),
            _ => {
                let time = self.timeline.position();
                let duration = self.timeline.duration(frames);
                self.writer
                    .lock()
                    .map_err(|_| Error::from(E_FAIL))?
                    .write_audio(pcm, time, duration)?;
                self.timeline.advance(frames);
                Ok(())
            }
        }
    }

    fn silence(&mut self, frames: u64) -> Result<()> {
        (0..frames.div_ceil(MAX_CHUNK_FRAMES)).try_for_each(|chunk| {
            let size = (frames - chunk * MAX_CHUNK_FRAMES).min(MAX_CHUNK_FRAMES);
            self.write(&vec![0u8; size as usize * BYTES_PER_FRAME])
        })
    }

    fn fill_idle(&mut self) -> Result<()> {
        let until = clock::now() - self.origin - FILL_LAG_TICKS;
        self.silence(self.timeline.silence_until(until))
    }

    fn packet(&mut self, pcm: &[u8], packet_time: i64) -> Result<()> {
        match self.timeline.align(packet_time) {
            Alignment::Contiguous => self.write(pcm),
            Alignment::Silence(frames) => self.silence(frames).and_then(|_| self.write(pcm)),
            Alignment::Skip(frames) => {
                let skip = (frames as usize * BYTES_PER_FRAME).min(pcm.len());
                self.write(&pcm[skip..])
            }
        }
    }

    fn drain(&mut self, loopback: &Loopback, running: &AtomicBool) -> Result<()> {
        while running.load(Ordering::Relaxed) {
            thread::sleep(POLL);
            while unsafe { loopback.capture.GetNextPacketSize()? } > 0 {
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                let mut qpc = 0u64;
                unsafe { loopback.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut qpc))? };
                let bytes = frames as usize * BYTES_PER_FRAME;
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null();
                let zeros;
                let pcm: &[u8] = match silent {
                    true => {
                        zeros = vec![0u8; bytes];
                        &zeros
                    }
                    false => unsafe { std::slice::from_raw_parts(data, bytes) },
                };
                let written = self.packet(pcm, qpc as i64 - self.origin);
                unsafe { loopback.capture.ReleaseBuffer(frames)? };
                written?;
            }
            self.fill_idle()?;
        }
        Ok(())
    }
}

pub fn pump(writer: Arc<Mutex<Writer>>, origin: i64, running: Arc<AtomicBool>) {
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let mut sink = Sink {
        writer,
        timeline: Timeline::new(SAMPLE_RATE, TICKS_PER_SECOND, TOLERANCE_TICKS),
        origin,
    };
    let mut reported = false;
    while running.load(Ordering::Relaxed) {
        let outcome = Loopback::open().and_then(|loopback| sink.drain(&loopback, &running));
        if let Err(error) = outcome {
            if !reported {
                eprintln!("recorder: audio interrupted: {error}");
                reported = true;
            }
            let _ = sink.fill_idle();
            thread::sleep(REOPEN);
        }
    }
}
