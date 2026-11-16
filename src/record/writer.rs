use std::path::Path;

use windows::{
    Win32::{
        Foundation::E_FAIL,
        Graphics::Direct3D11::{D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, ID3D11Texture2D},
        Media::MediaFoundation::{
            IMF2DBuffer, IMFDXGIBuffer, IMFMediaType,
            IMFSample, IMFSinkWriter, IMFVideoSampleAllocatorEx, MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
            MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS,
            MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
            MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_MAX_KEYFRAME_SPACING,
            MF_MT_MPEG2_PROFILE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
            MF_MT_VIDEO_NOMINAL_RANGE, MF_MT_YUV_MATRIX, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
            MF_SA_D3D11_BINDFLAGS, MF_SINK_WRITER_D3D_MANAGER, MFAudioFormat_AAC,
            MFAudioFormat_PCM, MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer,
            MFCreateSample, MFCreateSinkWriterFromURL, MFCreateVideoSampleAllocatorEx,
            MFMediaType_Audio, MFMediaType_Video, MFNominalRange_16_235,
            MFVideoFormat_H264, MFVideoFormat_NV12, MFVideoInterlace_Progressive,
            MFVideoTransferMatrix_BT709, eAVEncH264VProfile_High,
        },
    },
    core::{Error, GUID, HSTRING, Interface, Result},
};

use super::{
    AUDIO_BYTES_PER_SECOND, CHANNELS, FPS, HEIGHT, SAMPLE_RATE, VIDEO_BITRATE, WIDTH, gpu::Gpu,
};

const BYTES_PER_FRAME: u32 = CHANNELS as u32 * 2;
const KEYFRAME_INTERVAL: u32 = FPS * 2;
const MIN_SAMPLES: u32 = 4;
const MAX_SAMPLES: u32 = 16;

pub struct VideoSlot {
    pub sample: IMFSample,
    pub texture: ID3D11Texture2D,
    pub slice: u32,
}

pub struct Writer {
    sink: Option<IMFSinkWriter>,
    allocator: IMFVideoSampleAllocatorEx,
    video: u32,
    audio: u32,
    frames: u64,
}

unsafe impl Send for Writer {}

fn pack(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

fn video_type(subtype: &GUID) -> Result<IMFMediaType> {
    let kind = unsafe { MFCreateMediaType()? };
    unsafe {
        kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        kind.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        kind.SetUINT64(&MF_MT_FRAME_SIZE, pack(WIDTH, HEIGHT))?;
        kind.SetUINT64(&MF_MT_FRAME_RATE, pack(FPS, 1))?;
        kind.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        kind.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        kind.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
        kind.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
    }
    Ok(kind)
}

fn audio_type(subtype: &GUID, bytes_per_second: u32) -> Result<IMFMediaType> {
    let kind = unsafe { MFCreateMediaType()? };
    unsafe {
        kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        kind.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        kind.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        kind.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, SAMPLE_RATE)?;
        kind.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, u32::from(CHANNELS))?;
        kind.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, bytes_per_second)?;
    }
    Ok(kind)
}

impl Writer {
    pub fn create(gpu: &Gpu, path: &Path) -> Result<Self> {
        let attributes = {
            let mut created = None;
            unsafe { MFCreateAttributes(&mut created, 2)? };
            created.ok_or_else(|| Error::from(E_FAIL))?
        };
        unsafe {
            attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
            attributes.SetUnknown(&MF_SINK_WRITER_D3D_MANAGER, &gpu.manager)?;
        }
        let sink = unsafe { MFCreateSinkWriterFromURL(&HSTRING::from(path.as_os_str()), None, &attributes)? };

        let encoded = video_type(&MFVideoFormat_H264)?;
        unsafe {
            encoded.SetUINT32(&MF_MT_AVG_BITRATE, VIDEO_BITRATE)?;
            encoded.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32)?;
            encoded.SetUINT32(&MF_MT_MAX_KEYFRAME_SPACING, KEYFRAME_INTERVAL)?;
        }
        let video = unsafe { sink.AddStream(&encoded)? };
        let raw_video = video_type(&MFVideoFormat_NV12)?;
        unsafe { sink.SetInputMediaType(video, &raw_video, None)? };

        let audio = unsafe { sink.AddStream(&audio_type(&MFAudioFormat_AAC, AUDIO_BYTES_PER_SECOND)?)? };
        let raw_audio = audio_type(&MFAudioFormat_PCM, SAMPLE_RATE * BYTES_PER_FRAME)?;
        unsafe {
            raw_audio.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, BYTES_PER_FRAME)?;
            sink.SetInputMediaType(audio, &raw_audio, None)?;
        }

        let allocator = {
            let mut raw = std::ptr::null_mut();
            unsafe { MFCreateVideoSampleAllocatorEx(&IMFVideoSampleAllocatorEx::IID, &mut raw)? };
            unsafe { IMFVideoSampleAllocatorEx::from_raw(raw) }
        };
        let allocator_attributes = {
            let mut created = None;
            unsafe { MFCreateAttributes(&mut created, 1)? };
            created.ok_or_else(|| Error::from(E_FAIL))?
        };
        unsafe {
            allocator_attributes.SetUINT32(
                &MF_SA_D3D11_BINDFLAGS,
                (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            )?;
            allocator.SetDirectXManager(&gpu.manager)?;
            allocator.InitializeSampleAllocatorEx(MIN_SAMPLES, MAX_SAMPLES, &allocator_attributes, &raw_video)?;
            sink.BeginWriting()?;
        }
        Ok(Self {
            sink: Some(sink),
            allocator,
            video,
            audio,
            frames: 0,
        })
    }

    fn sink(&self) -> Result<&IMFSinkWriter> {
        self.sink.as_ref().ok_or_else(|| Error::from(E_FAIL))
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn video_slot(&self) -> Option<VideoSlot> {
        let sample = unsafe { self.allocator.AllocateSample() }.ok()?;
        let buffer = unsafe { sample.GetBufferByIndex(0) }.ok()?;
        let dxgi: IMFDXGIBuffer = buffer.cast().ok()?;
        let mut raw = std::ptr::null_mut();
        unsafe { dxgi.GetResource(&ID3D11Texture2D::IID, &mut raw) }.ok()?;
        let texture = unsafe { ID3D11Texture2D::from_raw(raw) };
        let slice = unsafe { dxgi.GetSubresourceIndex() }.ok()?;
        Some(VideoSlot {
            sample,
            texture,
            slice,
        })
    }

    pub fn write_video(&mut self, slot: &VideoSlot, time: i64, duration: i64) -> Result<()> {
        let buffer = unsafe { slot.sample.GetBufferByIndex(0)? };
        let length = unsafe { buffer.cast::<IMF2DBuffer>()?.GetContiguousLength()? };
        unsafe {
            buffer.SetCurrentLength(length)?;
            slot.sample.SetSampleTime(time)?;
            slot.sample.SetSampleDuration(duration)?;
            self.sink()?.WriteSample(self.video, &slot.sample)?;
        }
        self.frames += 1;
        Ok(())
    }

    pub fn write_audio(&self, pcm: &[u8], time: i64, duration: i64) -> Result<()> {
        let length = pcm.len() as u32;
        let buffer = unsafe { MFCreateMemoryBuffer(length)? };
        let mut target = std::ptr::null_mut();
        unsafe {
            buffer.Lock(&mut target, None, None)?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr(), target, pcm.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(length)?;
        }
        let sample = unsafe { MFCreateSample()? };
        unsafe {
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(time)?;
            sample.SetSampleDuration(duration)?;
            self.sink()?.WriteSample(self.audio, &sample)
        }
    }

    pub fn finish(&mut self) -> Result<u64> {
        let sink = self.sink.take().ok_or_else(|| Error::from(E_FAIL))?;
        match self.frames {
            0 => Ok(0),
            frames => unsafe { sink.Finalize() }.map(|_| frames),
        }
    }
}
