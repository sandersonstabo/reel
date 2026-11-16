use std::{
    mem::ManuallyDrop,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::Duration,
};

use windows::{
    Win32::{
        Foundation::E_FAIL,
        Graphics::Direct3D11::{D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE},
        Media::MediaFoundation::{
            CODECAPI_AVEncCommonMaxBitRate, CODECAPI_AVEncCommonMeanBitRate,
            CODECAPI_AVEncCommonRateControlMode, CODECAPI_AVEncMPVDefaultBPictureCount,
            CODECAPI_AVEncMPVGOPSize, ICodecAPI, IMFMediaEventGenerator, IMFSample, IMFShutdown,
            IMFTransform, IMFVideoSampleAllocatorEx, METransformHaveOutput, METransformNeedInput,
            MF_E_TRANSFORM_STREAM_CHANGE, MF_EVENT_FLAG_NONE, MF_MT_AVG_BITRATE,
            MF_MT_MPEG_SEQUENCE_HEADER, MF_MT_MPEG2_PROFILE, MF_SA_D3D11_BINDFLAGS,
            MF_TRANSFORM_ASYNC_UNLOCK, MFCreateVideoSampleAllocatorEx, MFMediaType_Video,
            MFSampleExtension_CleanPoint, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE,
            MFT_ENUM_FLAG_SORTANDFILTER, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,
            MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_MESSAGE_NOTIFY_START_OF_STREAM,
            MFT_MESSAGE_SET_D3D_MANAGER, MFT_OUTPUT_DATA_BUFFER, MFT_OUTPUT_STREAM_PROVIDES_SAMPLES,
            MFVideoFormat_AV1, MFVideoFormat_H264, MFVideoFormat_HEVC, MFVideoFormat_NV12,
            eAVEncCommonRateControlMode_CBR, eAVEncCommonRateControlMode_PeakConstrainedVBR,
            eAVEncH264VProfile_High, eAVEncH265VProfile_Main_420_8,
        },
    },
    core::{Error, GUID, Interface, Result},
};

use super::{media, mft};
use crate::{
    mux::clip,
    record::{
        TICKS_PER_SECOND,
        plan::{KEYFRAME_SECONDS, Plan, RateControl},
        ring::{Packet, Ring},
    },
    settings::Codec,
};

const PEAK_FACTOR: f64 = 1.6;
const NEED_INPUT: i32 = METransformNeedInput.0;
const HAVE_OUTPUT: i32 = METransformHaveOutput.0;
const INPUT_POLL: Duration = Duration::from_millis(100);
const MIN_SAMPLES: u32 = 4;
const MAX_SAMPLES: u32 = 12;

pub struct Frame(pub IMFSample);

unsafe impl Send for Frame {}

pub struct VideoEncoder {
    transform: IMFTransform,
    events: IMFMediaEventGenerator,
    codec: Codec,
    interval: i64,
    pub name: String,
}

unsafe impl Send for VideoEncoder {}

fn subtype(codec: Codec) -> GUID {
    match codec {
        Codec::H264 => MFVideoFormat_H264,
        Codec::Hevc => MFVideoFormat_HEVC,
        Codec::Av1 => MFVideoFormat_AV1,
    }
}

fn profile(codec: Codec) -> Option<u32> {
    match codec {
        Codec::H264 => Some(eAVEncH264VProfile_High.0 as u32),
        Codec::Hevc => Some(eAVEncH265VProfile_Main_420_8.0 as u32),
        Codec::Av1 => None,
    }
}

fn candidates(codec: Codec) -> Result<Vec<windows::Win32::Media::MediaFoundation::IMFActivate>> {
    mft::find(
        MFT_CATEGORY_VIDEO_ENCODER,
        MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
        (MFMediaType_Video, MFVideoFormat_NV12),
        (MFMediaType_Video, subtype(codec)),
    )
}

pub fn available(codec: Codec) -> bool {
    candidates(codec).is_ok_and(|found| !found.is_empty())
}

fn rate_settings(plan: &Plan) -> Vec<(GUID, u32)> {
    let gop = plan.fps * KEYFRAME_SECONDS;
    let common = vec![
        (CODECAPI_AVEncMPVGOPSize, gop),
        (CODECAPI_AVEncMPVDefaultBPictureCount, 0),
        (CODECAPI_AVEncCommonMeanBitRate, plan.video_bitrate),
    ];
    let mode = match plan.rate_control {
        RateControl::Constant => vec![(CODECAPI_AVEncCommonRateControlMode, eAVEncCommonRateControlMode_CBR.0 as u32)],
        RateControl::Quality => vec![
            (
                CODECAPI_AVEncCommonRateControlMode,
                eAVEncCommonRateControlMode_PeakConstrainedVBR.0 as u32,
            ),
            (CODECAPI_AVEncCommonMaxBitRate, (f64::from(plan.video_bitrate) * PEAK_FACTOR) as u32),
        ],
    };
    [mode, common].concat()
}

pub fn allocator(manager: &windows::Win32::Media::MediaFoundation::IMFDXGIDeviceManager, plan: &Plan) -> Result<IMFVideoSampleAllocatorEx> {
    let allocator = {
        let mut raw = std::ptr::null_mut();
        unsafe { MFCreateVideoSampleAllocatorEx(&IMFVideoSampleAllocatorEx::IID, &mut raw)? };
        unsafe { IMFVideoSampleAllocatorEx::from_raw(raw) }
    };
    let settings = media::attributes(1)?;
    unsafe {
        settings.SetUINT32(
            &MF_SA_D3D11_BINDFLAGS,
            (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        )?;
        allocator.SetDirectXManager(manager)?;
        allocator.InitializeSampleAllocatorEx(
            MIN_SAMPLES,
            MAX_SAMPLES,
            &settings,
            &media::video(&MFVideoFormat_NV12, plan.resolution, plan.fps)?,
        )?;
    }
    Ok(allocator)
}

impl VideoEncoder {
    pub fn create(manager: &windows::Win32::Media::MediaFoundation::IMFDXGIDeviceManager, plan: &Plan) -> Result<(Self, Vec<GUID>)> {
        let activate = candidates(plan.codec)?
            .into_iter()
            .next()
            .ok_or_else(|| Error::new(E_FAIL, format!("no hardware {} encoder", plan.codec.key())))?;
        let name = mft::name(&activate);
        let transform = mft::activate(&activate)?;
        unsafe {
            transform.GetAttributes()?.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1)?;
            transform.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, manager.as_raw() as usize)?;
        }
        let rejected = match transform.cast::<ICodecAPI>() {
            Ok(codec) => mft::configure(&codec, &rate_settings(plan)),
            Err(_) => vec![CODECAPI_AVEncCommonRateControlMode],
        };
        let output = media::video(&subtype(plan.codec), plan.resolution, plan.fps)?;
        unsafe {
            output.SetUINT32(&MF_MT_AVG_BITRATE, plan.video_bitrate)?;
            if let Some(profile) = profile(plan.codec) {
                output.SetUINT32(&MF_MT_MPEG2_PROFILE, profile)?;
            }
            transform.SetOutputType(0, &output, 0)?;
            transform.SetInputType(0, &media::video(&MFVideoFormat_NV12, plan.resolution, plan.fps)?, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        }
        Ok((
            Self {
                events: transform.cast()?,
                transform,
                codec: plan.codec,
                interval: TICKS_PER_SECOND / i64::from(plan.fps),
                name,
            },
            rejected,
        ))
    }

    fn sequence_header(&self) -> Option<Vec<u8>> {
        let current = unsafe { self.transform.GetOutputCurrentType(0) }.ok()?;
        let size = unsafe { current.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER) }.ok()?;
        let mut blob = vec![0u8; size as usize];
        unsafe { current.GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut blob, None) }.ok()?;
        Some(blob)
    }

    fn output(&self) -> Result<Option<Packet>> {
        let info = unsafe { self.transform.GetOutputStreamInfo(0)? };
        let provided = info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;
        let supplied = match provided {
            true => None,
            false => Some(media::memory_sample(&[], info.cbSize)?),
        };
        let mut buffer = MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: ManuallyDrop::new(supplied),
            dwStatus: 0,
            pEvents: ManuallyDrop::new(None),
        };
        let mut status = 0u32;
        let result = unsafe { self.transform.ProcessOutput(0, std::slice::from_mut(&mut buffer), &mut status) };
        let sample = ManuallyDrop::into_inner(buffer.pSample);
        drop(ManuallyDrop::into_inner(buffer.pEvents));
        match result {
            Err(error) if error.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                let offered = unsafe { self.transform.GetOutputAvailableType(0, 0)? };
                unsafe { self.transform.SetOutputType(0, &offered, 0)? };
                Ok(None)
            }
            Err(error) => Err(error),
            Ok(()) => sample.map(|sample| self.packet(&sample)).transpose(),
        }
    }

    fn packet(&self, sample: &IMFSample) -> Result<Packet> {
        let data = media::sample_bytes(sample)?;
        let flagged = unsafe { sample.GetUINT32(&MFSampleExtension_CleanPoint) }.is_ok_and(|value| value != 0);
        Ok(Packet {
            time: unsafe { sample.GetSampleTime()? },
            duration: unsafe { sample.GetSampleDuration() }.unwrap_or(self.interval),
            keyframe: flagged || clip::is_keyframe(self.codec, &data),
            data: data.into(),
        })
    }

    fn feed(&self, frames: &Receiver<Frame>, running: &AtomicBool) -> Result<bool> {
        loop {
            match (running.load(Ordering::Relaxed), frames.recv_timeout(INPUT_POLL)) {
                (false, _) | (_, Err(RecvTimeoutError::Disconnected)) => return Ok(false),
                (true, Err(RecvTimeoutError::Timeout)) => continue,
                (true, Ok(Frame(sample))) => {
                    unsafe { self.transform.ProcessInput(0, &sample, 0)? };
                    return Ok(true);
                }
            }
        }
    }

    pub fn pump(
        &self,
        frames: Receiver<Frame>,
        ring: Arc<Mutex<Ring>>,
        header: Arc<Mutex<Option<Vec<u8>>>>,
        running: Arc<AtomicBool>,
    ) -> Result<()> {
        loop {
            let event = unsafe { self.events.GetEvent(MF_EVENT_FLAG_NONE)? };
            match unsafe { event.GetType()? } as i32 {
                NEED_INPUT => {
                    if !self.feed(&frames, &running)? {
                        return Ok(());
                    }
                }
                HAVE_OUTPUT => {
                    if let Some(packet) = self.output()? {
                        if let Ok(mut known) = header.lock() {
                            if known.is_none() {
                                *known = self.sequence_header();
                            }
                        }
                        ring.lock().map_err(|_| Error::from(E_FAIL))?.push_video(packet);
                    }
                }
                _ => {}
            }
        }
    }
}

impl Drop for VideoEncoder {
    fn drop(&mut self) {
        unsafe {
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            if let Ok(shutdown) = self.transform.cast::<IMFShutdown>() {
                let _ = shutdown.Shutdown();
            }
        }
    }
}
