use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU16, Ordering},
        mpsc::{self, Sender},
    },
    thread,
    time::Duration,
};

use windows::{
    Win32::{
        Foundation::E_FAIL,
        Media::Audio::{
            AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR, AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_LOOPBACK,
            AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, AUDIOCLIENT_ACTIVATION_PARAMS,
            AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation,
            IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
            IAudioCaptureClient, IAudioClient, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVE_FORMAT_PCM, WAVEFORMATEX,
        },
        System::{
            Com::{BLOB, COINIT_MULTITHREADED, CoInitializeEx, IAgileObject, StructuredStorage::PROPVARIANT},
            Variant::VT_BLOB,
        },
    },
    core::{Error, HRESULT, IUnknown, Interface, Ref, Result, implement},
};

use super::{
    CHANNELS, SAMPLE_RATE, TICKS_PER_SECOND,
    clock,
    encode::audio::AudioEncoder,
    level,
    ring::Ring,
    timeline::{Alignment, Timeline},
};

const BYTES_PER_FRAME: usize = CHANNELS as usize * 2;
const BUFFER_TICKS: i64 = TICKS_PER_SECOND / 5;
const TOLERANCE_TICKS: i64 = TICKS_PER_SECOND / 50;
const FILL_LAG_TICKS: i64 = TICKS_PER_SECOND / 10;
const POLL: Duration = Duration::from_millis(10);
const REOPEN: Duration = Duration::from_millis(500);
const MAX_CHUNK_FRAMES: u64 = SAMPLE_RATE as u64;
const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(5);

#[implement(IActivateAudioInterfaceCompletionHandler, IAgileObject)]
struct Activation(Mutex<Option<Sender<()>>>);

impl windows::Win32::System::Com::IAgileObject_Impl for Activation_Impl {}

impl IActivateAudioInterfaceCompletionHandler_Impl for Activation_Impl {
    fn ActivateCompleted(&self, _operation: Ref<IActivateAudioInterfaceAsyncOperation>) -> Result<()> {
        if let Some(done) = self.0.lock().ok().and_then(|mut slot| slot.take()) {
            let _ = done.send(());
        }
        Ok(())
    }
}

fn activate(process: u32) -> Result<IAudioClient> {
    let parameters = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: process,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            },
        },
    };
    let mut variant = std::mem::ManuallyDrop::new(PROPVARIANT::default());
    unsafe {
        let inner = &mut *variant.Anonymous.Anonymous;
        inner.vt = VT_BLOB;
        inner.Anonymous.blob = BLOB {
            cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
            pBlobData: &parameters as *const _ as *mut u8,
        };
    }
    let (done, completed) = mpsc::channel();
    let handler: IActivateAudioInterfaceCompletionHandler = Activation(Mutex::new(Some(done))).into();
    let operation = unsafe {
        ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, &IAudioClient::IID, Some(&*variant), &handler)?
    };
    completed
        .recv_timeout(ACTIVATION_TIMEOUT)
        .map_err(|_| Error::new(E_FAIL, "process audio activation timed out"))?;
    let mut status = HRESULT(0);
    let mut activated: Option<IUnknown> = None;
    unsafe { operation.GetActivateResult(&mut status, &mut activated)? };
    status.ok()?;
    activated.ok_or_else(|| Error::from(E_FAIL))?.cast()
}

struct Loopback {
    client: IAudioClient,
    capture: IAudioCaptureClient,
}

impl Loopback {
    fn open(process: u32) -> Result<Self> {
        let client = activate(process)?;
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
    encoder: AudioEncoder,
    ring: Arc<Mutex<Ring>>,
    peak: Arc<AtomicU16>,
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
                let packets = self.encoder.encode(pcm, time, duration)?;
                let mut ring = self.ring.lock().map_err(|_| Error::from(E_FAIL))?;
                packets.into_iter().for_each(|packet| ring.push_audio(packet));
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
        self.peak.fetch_max(level::peak(pcm), Ordering::Relaxed);
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
                let trusted = qpc != 0 && flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 == 0;
                let packet_time = match trusted {
                    true => qpc as i64 - self.origin,
                    false => clock::now() - self.origin - self.timeline.duration(u64::from(frames)),
                };
                let zeros;
                let pcm: &[u8] = match silent {
                    true => {
                        zeros = vec![0u8; bytes];
                        &zeros
                    }
                    false => unsafe { std::slice::from_raw_parts(data, bytes) },
                };
                let written = self.packet(pcm, packet_time);
                unsafe { loopback.capture.ReleaseBuffer(frames)? };
                written?;
            }
            self.fill_idle()?;
        }
        Ok(())
    }
}

pub fn pump(
    process: u32,
    ring: Arc<Mutex<Ring>>,
    peak: Arc<AtomicU16>,
    bitrate: u32,
    origin: i64,
    running: Arc<AtomicBool>,
) {
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let encoder = match AudioEncoder::create(bitrate) {
        Ok(encoder) => encoder,
        Err(error) => {
            eprintln!("recorder: no audio encoder: {error}");
            return;
        }
    };
    let mut sink = Sink {
        encoder,
        ring,
        peak,
        timeline: Timeline::new(SAMPLE_RATE, TICKS_PER_SECOND, TOLERANCE_TICKS),
        origin,
    };
    let mut reported = false;
    while running.load(Ordering::Relaxed) {
        let outcome = Loopback::open(process).and_then(|loopback| sink.drain(&loopback, &running));
        if let Err(error) = outcome {
            if !reported {
                eprintln!("recorder: game audio unavailable for process {process}: {error}");
                reported = true;
            }
            let _ = sink.fill_idle();
            thread::sleep(REOPEN);
        }
    }
}
