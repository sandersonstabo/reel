use windows::{
    Win32::{
        Foundation::E_FAIL,
        Media::MediaFoundation::{
            IMFAttributes, IMFMediaType, IMFSample, MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
            MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
            MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE,
            MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE, MF_MT_TRANSFER_FUNCTION,
            MF_MT_VIDEO_NOMINAL_RANGE, MF_MT_VIDEO_PRIMARIES, MF_MT_YUV_MATRIX, MFCreateAttributes,
            MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample, MFMediaType_Audio,
            MFMediaType_Video, MFNominalRange_16_235, MFVideoInterlace_Progressive,
            MFVideoPrimaries_BT709, MFVideoTransFunc_709, MFVideoTransferMatrix_BT709,
        },
    },
    core::{Error, GUID, Result},
};

use crate::record::{CHANNELS, SAMPLE_RATE, plan::Resolution};

pub fn pack(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

pub fn attributes(capacity: u32) -> Result<IMFAttributes> {
    let mut created = None;
    unsafe { MFCreateAttributes(&mut created, capacity)? };
    created.ok_or_else(|| Error::from(E_FAIL))
}

pub fn video(subtype: &GUID, resolution: Resolution, fps: u32) -> Result<IMFMediaType> {
    let kind = unsafe { MFCreateMediaType()? };
    unsafe {
        kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        kind.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        kind.SetUINT64(&MF_MT_FRAME_SIZE, pack(resolution.width, resolution.height))?;
        kind.SetUINT64(&MF_MT_FRAME_RATE, pack(fps, 1))?;
        kind.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        kind.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        kind.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
        kind.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
        kind.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32)?;
        kind.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32)?;
    }
    Ok(kind)
}

pub fn audio(subtype: &GUID, bytes_per_second: u32) -> Result<IMFMediaType> {
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

pub fn memory_sample(bytes: &[u8], capacity: u32) -> Result<IMFSample> {
    let buffer = unsafe { MFCreateMemoryBuffer(capacity.max(bytes.len() as u32))? };
    if !bytes.is_empty() {
        let mut target = std::ptr::null_mut();
        unsafe {
            buffer.Lock(&mut target, None, None)?;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(bytes.len() as u32)?;
        }
    }
    let sample = unsafe { MFCreateSample()? };
    unsafe { sample.AddBuffer(&buffer)? };
    Ok(sample)
}

pub fn sample_bytes(sample: &IMFSample) -> Result<Vec<u8>> {
    let buffer = unsafe { sample.ConvertToContiguousBuffer()? };
    let mut source = std::ptr::null_mut();
    let mut length = 0u32;
    unsafe { buffer.Lock(&mut source, None, Some(&mut length))? };
    let bytes = unsafe { std::slice::from_raw_parts(source, length as usize) }.to_vec();
    unsafe { buffer.Unlock()? };
    Ok(bytes)
}
