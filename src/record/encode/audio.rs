use std::mem::ManuallyDrop;

use windows::{
    Win32::{
        Foundation::E_FAIL,
        Media::MediaFoundation::{
            IMFTransform, MF_E_TRANSFORM_NEED_MORE_INPUT, MF_MT_AAC_PAYLOAD_TYPE,
            MF_MT_AUDIO_BLOCK_ALIGNMENT, MFAudioFormat_AAC, MFAudioFormat_PCM, MFMediaType_Audio,
            MFT_CATEGORY_AUDIO_ENCODER, MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_FLAG_SYNCMFT,
            MFT_OUTPUT_DATA_BUFFER,
        },
    },
    core::{Error, Result},
};

use super::{media, mft};
use crate::record::{CHANNELS, SAMPLE_RATE, ring::Packet};

const BYTES_PER_FRAME: u32 = CHANNELS as u32 * 2;

pub struct AudioEncoder {
    transform: IMFTransform,
    output_size: u32,
}

impl AudioEncoder {
    pub fn create(bitrate: u32) -> Result<Self> {
        let activate = mft::find(
            MFT_CATEGORY_AUDIO_ENCODER,
            MFT_ENUM_FLAG_SYNCMFT | MFT_ENUM_FLAG_SORTANDFILTER,
            (MFMediaType_Audio, MFAudioFormat_PCM),
            (MFMediaType_Audio, MFAudioFormat_AAC),
        )?
        .into_iter()
        .next()
        .ok_or_else(|| Error::new(E_FAIL, "no AAC encoder"))?;
        let transform = mft::activate(&activate)?;
        let output = media::audio(&MFAudioFormat_AAC, bitrate / 8)?;
        let input = media::audio(&MFAudioFormat_PCM, SAMPLE_RATE * BYTES_PER_FRAME)?;
        unsafe {
            output.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?;
            input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, BYTES_PER_FRAME)?;
            transform.SetOutputType(0, &output, 0)?;
            transform.SetInputType(0, &input, 0)?;
        }
        let output_size = unsafe { transform.GetOutputStreamInfo(0)? }.cbSize;
        Ok(Self { transform, output_size })
    }

    fn drain(&self) -> Result<Vec<Packet>> {
        let mut packets = Vec::new();
        loop {
            let mut buffer = MFT_OUTPUT_DATA_BUFFER {
                dwStreamID: 0,
                pSample: ManuallyDrop::new(Some(media::memory_sample(&[], self.output_size)?)),
                dwStatus: 0,
                pEvents: ManuallyDrop::new(None),
            };
            let mut status = 0u32;
            let result = unsafe { self.transform.ProcessOutput(0, std::slice::from_mut(&mut buffer), &mut status) };
            let sample = ManuallyDrop::into_inner(buffer.pSample);
            drop(ManuallyDrop::into_inner(buffer.pEvents));
            match (result, sample) {
                (Err(error), _) if error.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(packets),
                (Err(error), _) => return Err(error),
                (Ok(()), None) => return Ok(packets),
                (Ok(()), Some(sample)) => packets.push(Packet {
                    time: unsafe { sample.GetSampleTime()? },
                    duration: unsafe { sample.GetSampleDuration()? },
                    keyframe: true,
                    data: media::sample_bytes(&sample)?.into(),
                }),
            }
        }
    }

    pub fn encode(&self, pcm: &[u8], time: i64, duration: i64) -> Result<Vec<Packet>> {
        let sample = media::memory_sample(pcm, pcm.len() as u32)?;
        unsafe {
            sample.SetSampleTime(time)?;
            sample.SetSampleDuration(duration)?;
            self.transform.ProcessInput(0, &sample, 0)?;
        }
        self.drain()
    }
}
