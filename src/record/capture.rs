use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{Sender, SyncSender, TrySendError},
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
        Media::MediaFoundation::{IMF2DBuffer, IMFDXGIBuffer, IMFVideoSampleAllocatorEx},
        System::WinRT::{
            Direct3D11::IDirect3DDxgiInterfaceAccess, Graphics::Capture::IGraphicsCaptureItemInterop,
        },
    },
    core::{Interface, Result, factory},
};

use super::{
    TICKS_PER_SECOND,
    convert::Converter,
    encode::video::Frame,
    gpu::Gpu,
    pacing::Pacer,
    plan::Plan,
};

const POOL_BUFFERS: i32 = 2;
const MAX_REPORTED_ERRORS: u32 = 5;

#[derive(Default)]
pub struct Stats {
    pub arrived: AtomicU64,
    pub paced_out: AtomicU64,
    pub dropped: AtomicU64,
}

impl Stats {
    pub fn summary(&self) -> String {
        format!(
            "arrived {}, thinned by pacer {}, dropped while the encoder was busy {}",
            self.arrived.load(Ordering::Relaxed),
            self.paced_out.load(Ordering::Relaxed),
            self.dropped.load(Ordering::Relaxed),
        )
    }
}

struct Pipeline {
    stats: Arc<Stats>,
    gpu: Gpu,
    plan: Plan,
    device: IDirect3DDevice,
    allocator: IMFVideoSampleAllocatorEx,
    frames: SyncSender<Frame>,
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
    fn drop_frame(&self) -> Result<()> {
        self.stats.dropped.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

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
        let Ok(sample) = (unsafe { self.allocator.AllocateSample() }) else {
            return self.drop_frame();
        };
        let converter = match self.converter.take() {
            Some(existing) if existing.source() == source => existing,
            _ => Converter::new(&self.gpu, source, &self.plan)?,
        };
        let buffer = unsafe { sample.GetBufferByIndex(0)? };
        let surface: IMFDXGIBuffer = buffer.cast()?;
        let mut raw = std::ptr::null_mut();
        unsafe { surface.GetResource(&ID3D11Texture2D::IID, &mut raw)? };
        let target = unsafe { ID3D11Texture2D::from_raw(raw) };
        let converted = converter.convert(&texture, &target, unsafe { surface.GetSubresourceIndex()? });
        self.converter = Some(converter);
        converted?;
        unsafe {
            buffer.SetCurrentLength(buffer.cast::<IMF2DBuffer>()?.GetContiguousLength()?)?;
            sample.SetSampleTime(time)?;
            sample.SetSampleDuration(self.pacer.interval())?;
        }
        match self.frames.try_send(Frame(sample)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => self.drop_frame(),
        }
    }

    fn report(&mut self, error: &windows::core::Error) {
        if self.errors < MAX_REPORTED_ERRORS {
            eprintln!("recorder: frame failed: {error}");
        }
        self.errors += 1;
    }
}

pub fn item(window: HWND) -> Result<GraphicsCaptureItem> {
    let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    unsafe { interop.CreateForWindow(window) }
}

pub struct Capture {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    _item: GraphicsCaptureItem,
}

pub struct Wiring {
    pub allocator: IMFVideoSampleAllocatorEx,
    pub frames: SyncSender<Frame>,
    pub closed: Sender<()>,
    pub stats: Arc<Stats>,
    pub origin: i64,
}

impl Capture {
    pub fn start(gpu: &Gpu, item: GraphicsCaptureItem, plan: &Plan, wiring: Wiring) -> Result<Self> {
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
        let Wiring {
            allocator,
            frames,
            closed,
            stats,
            origin,
        } = wiring;
        let pipeline = Mutex::new(Pipeline {
            stats,
            gpu: gpu.clone(),
            plan: *plan,
            device,
            allocator,
            frames,
            converter: None,
            pacer: Pacer::new(TICKS_PER_SECOND, plan.fps),
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
