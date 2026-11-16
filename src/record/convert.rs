use std::mem::ManuallyDrop;

use windows::{
    Win32::{
        Foundation::RECT,
        Graphics::{
            Direct3D11::{
                D3D11_TEX2D_ARRAY_VPOV, D3D11_TEX2D_VPIV, D3D11_TEX2D_VPOV, D3D11_TEXTURE2D_DESC,
                D3D11_VIDEO_COLOR, D3D11_VIDEO_COLOR_0, D3D11_VIDEO_COLOR_YCbCrA,
                D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_COLOR_SPACE,
                D3D11_VIDEO_PROCESSOR_CONTENT_DESC, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC,
                D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC,
                D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_STREAM,
                D3D11_VIDEO_USAGE_PLAYBACK_NORMAL, D3D11_VPIV_DIMENSION_TEXTURE2D,
                D3D11_VPOV_DIMENSION_TEXTURE2D, D3D11_VPOV_DIMENSION_TEXTURE2DARRAY,
                ID3D11Texture2D, ID3D11VideoProcessor, ID3D11VideoProcessorEnumerator,
            },
            Dxgi::Common::DXGI_RATIONAL,
        },
    },
    core::Result,
};

use super::{
    gpu::Gpu,
    layout::{self, Rect},
    plan::Plan,
};

const RGB_FULL_RANGE: u32 = 0;
const YCBCR_BT709_STUDIO: u32 = (1 << 2) | (1 << 4);

pub struct Converter {
    gpu: Gpu,
    enumerator: ID3D11VideoProcessorEnumerator,
    processor: ID3D11VideoProcessor,
    source: (u32, u32),
}

unsafe impl Send for Converter {}

fn rect(value: Rect) -> RECT {
    RECT {
        left: value.left,
        top: value.top,
        right: value.right,
        bottom: value.bottom,
    }
}

impl Converter {
    pub fn new(gpu: &Gpu, source: (u32, u32), plan: &Plan) -> Result<Self> {
        let output = (plan.resolution.width, plan.resolution.height);
        let rate = DXGI_RATIONAL {
            Numerator: plan.fps,
            Denominator: 1,
        };
        let content = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: rate,
            InputWidth: source.0,
            InputHeight: source.1,
            OutputFrameRate: rate,
            OutputWidth: output.0,
            OutputHeight: output.1,
            Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
        };
        let enumerator = unsafe { gpu.video_device.CreateVideoProcessorEnumerator(&content)? };
        let processor = unsafe { gpu.video_device.CreateVideoProcessor(&enumerator, 0)? };
        let context = &gpu.video_context;
        let source_rect = rect(Rect {
            left: 0,
            top: 0,
            right: source.0 as i32,
            bottom: source.1 as i32,
        });
        let target = rect(layout::fit(source, output));
        let black = D3D11_VIDEO_COLOR {
            Anonymous: D3D11_VIDEO_COLOR_0 {
                YCbCr: D3D11_VIDEO_COLOR_YCbCrA {
                    Y: 16.0 / 255.0,
                    Cb: 0.5,
                    Cr: 0.5,
                    A: 1.0,
                },
            },
        };
        unsafe {
            context.VideoProcessorSetStreamFrameFormat(&processor, 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
            context.VideoProcessorSetStreamColorSpace(
                &processor,
                0,
                &D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: RGB_FULL_RANGE },
            );
            context.VideoProcessorSetOutputColorSpace(
                &processor,
                &D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: YCBCR_BT709_STUDIO },
            );
            context.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
            context.VideoProcessorSetStreamSourceRect(&processor, 0, true, Some(&source_rect));
            context.VideoProcessorSetStreamDestRect(&processor, 0, true, Some(&target));
            context.VideoProcessorSetOutputBackgroundColor(&processor, true, &black);
        }
        Ok(Self {
            gpu: gpu.clone(),
            enumerator,
            processor,
            source,
        })
    }

    pub fn source(&self) -> (u32, u32) {
        self.source
    }

    pub fn convert(&self, input: &ID3D11Texture2D, output: &ID3D11Texture2D, slice: u32) -> Result<()> {
        let input_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV {
                    MipSlice: 0,
                    ArraySlice: 0,
                },
            },
        };
        let mut texture = D3D11_TEXTURE2D_DESC::default();
        unsafe { output.GetDesc(&mut texture) };
        let output_desc = match texture.ArraySize > 1 {
            true => D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2DARRAY,
                Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                    Texture2DArray: D3D11_TEX2D_ARRAY_VPOV {
                        MipSlice: 0,
                        FirstArraySlice: slice,
                        ArraySize: 1,
                    },
                },
            },
            false => D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                    Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
                },
            },
        };
        let mut input_view = None;
        let mut output_view = None;
        unsafe {
            self.gpu.video_device.CreateVideoProcessorInputView(
                input,
                &self.enumerator,
                &input_desc,
                Some(&mut input_view),
            )?;
            self.gpu.video_device.CreateVideoProcessorOutputView(
                output,
                &self.enumerator,
                &output_desc,
                Some(&mut output_view),
            )?;
        }
        let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            pInputSurface: ManuallyDrop::new(input_view),
            ..Default::default()
        };
        let output_view = output_view.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
        let blitted = unsafe {
            self.gpu
                .video_context
                .VideoProcessorBlt(&self.processor, &output_view, 0, std::slice::from_ref(&stream))
        };
        unsafe { ManuallyDrop::drop(&mut stream.pInputSurface) };
        blitted
    }
}
