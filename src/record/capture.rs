use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::Sender,
};

use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
        SizeInt32,
    },
    Win32::{
        Foundation::HWND,
        Graphics::Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Texture2D},
        System::WinRT::{
            Direct3D11::IDirect3DDxgiInterfaceAccess, Graphics::Capture::IGraphicsCaptureItemInterop,
        },
    },
    core::{Interface, Result, factory},
};

use super::{FPS, TICKS_PER_SECOND, convert::Converter, gpu::Gpu, pacing::Pacer, writer::Writer};

const POOL_BUFFERS: i32 = 2;
const MAX_REPORTED_ERRORS: u32 = 5;

#[derive(Default)]
pub struct Stats {
    pub arrived: AtomicU64,
    pub paced_out: AtomicU64,
    pub no_buffer: AtomicU64,
}

impl Stats {
    pub fn summary(&self) -> String {
        format!(
            "arrived {}, thinned by pacer {}, dropped for lack of encoder buffer {}",
            self.arrived.load(Ordering::Relaxed),
            self.paced_out.load(Ordering::Relaxed),
            self.no_buffer.load(Ordering::Relaxed),
        )
    }
}

struct Pipeline {
    stats: Arc<Stats>,
    gpu: Gpu,
    device: IDirect3DDevice,
    writer: Arc<Mutex<Writer>>,
    converter: Option<Converter>,
    pacer: Pacer,
    origin: i64,
    pool_size: SizeInt32,
    errors: u32,
}

unsafe impl Send for Pipeline {}

fn texture_size(texture: &ID3D11Texture2D) -> (u32, u32) {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };
    (desc.Width, desc.Height)
}

impl Pipeline {
    fn on_frame(&mut self, pool: &Direct3D11CaptureFramePool) -> Result<()> {
        let frame = pool.TryGetNextFrame()?;
        let content = frame.ContentSize()?;
        if content.Width != self.pool_size.Width || content.Height != self.pool_size.Height {
            pool.Recreate(&self.device, DirectXPixelFormat::B8G8R8A8UIntNormalized, POOL_BUFFERS, content)?;
            self.pool_size = content;
        }
        self.stats.arrived.fetch_add(1, Ordering::Relaxed);
        let time = frame.SystemRelativeTime()?.Duration - self.origin;
        if !self.pacer.admit(time) {
            self.stats.paced_out.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        let texture: ID3D11Texture2D = unsafe { frame.Surface()?.cast::<IDirect3DDxgiInterfaceAccess>()?.GetInterface()? };
        let (texture_width, texture_height) = texture_size(&texture);
        let source = (
            (content.Width.max(0) as u32).min(texture_width),
            (content.Height.max(0) as u32).min(texture_height),
        );
        if source.0 < 2 || source.1 < 2 {
            return Ok(());
        }
        let converter = match self.converter.take() {
            Some(existing) if existing.source() == source => existing,
            _ => Converter::new(&self.gpu, source)?,
        };
        let mut writer = self.writer.lock().map_err(|_| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
        let result = match writer.video_slot() {
            None => {
                self.stats.no_buffer.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Some(slot) => converter
                .convert(&texture, &slot.texture, slot.slice)
                .and_then(|_| writer.write_video(&slot, time, self.pacer.interval())),
        };
        self.converter = Some(converter);
        result
    }

    fn report(&mut self, error: &windows::core::Error) {
        if self.errors < MAX_REPORTED_ERRORS {
            eprintln!("recorder: frame failed: {error}");
        }
        self.errors += 1;
    }
}

pub struct Capture {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    _item: GraphicsCaptureItem,
}

impl Capture {
    pub fn start(
        gpu: &Gpu,
        window: HWND,
        writer: Arc<Mutex<Writer>>,
        origin: i64,
        closed: Sender<()>,
        stats: Arc<Stats>,
    ) -> Result<Self> {
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(window)? };
        let device = gpu.winrt()?;
        let size = item.Size()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            POOL_BUFFERS,
            size,
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        let _ = session.SetIsBorderRequired(false);
        let _ = session.SetIsCursorCaptureEnabled(false);
        let pipeline = Mutex::new(Pipeline {
            stats,
            gpu: gpu.clone(),
            device,
            writer,
            converter: None,
            pacer: Pacer::new(TICKS_PER_SECOND, FPS),
            origin,
            pool_size: size,
            errors: 0,
        });
        pool.FrameArrived(&TypedEventHandler::new(
            move |pool: windows::core::Ref<Direct3D11CaptureFramePool>, _| {
                if let (Some(pool), Ok(mut pipeline)) = (pool.as_ref(), pipeline.lock()) {
                    if let Err(error) = pipeline.on_frame(pool) {
                        pipeline.report(&error);
                    }
                }
                Ok(())
            },
        ))?;
        item.Closed(&TypedEventHandler::new(move |_, _| {
            let _ = closed.send(());
            Ok(())
        }))?;
        session.StartCapture()?;
        Ok(Self {
            pool,
            session,
            _item: item,
        })
    }

    pub fn stop(self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
