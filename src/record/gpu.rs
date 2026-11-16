use windows::{
    Graphics::DirectX::Direct3D11::IDirect3DDevice,
    Win32::{
        Foundation::{E_FAIL, HMODULE},
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1},
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
                ID3D11Multithread, ID3D11VideoContext, ID3D11VideoDevice,
            },
            Dxgi::IDXGIDevice,
        },
        Media::MediaFoundation::{IMFDXGIDeviceManager, MFCreateDXGIDeviceManager},
        System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice,
    },
    core::{Error, Interface, Result},
};

#[derive(Clone)]
pub struct Gpu {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub video_device: ID3D11VideoDevice,
    pub video_context: ID3D11VideoContext,
    pub manager: IMFDXGIDeviceManager,
}

unsafe impl Send for Gpu {}

impl Gpu {
    pub fn create() -> Result<Self> {
        let mut device = None;
        let mut context = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?
        };
        let device: ID3D11Device = device.ok_or_else(|| Error::from(E_FAIL))?;
        let context: ID3D11DeviceContext = context.ok_or_else(|| Error::from(E_FAIL))?;
        let _ = unsafe { device.cast::<ID3D11Multithread>()?.SetMultithreadProtected(true) };
        let mut token = 0u32;
        let mut manager = None;
        unsafe { MFCreateDXGIDeviceManager(&mut token, &mut manager)? };
        let manager: IMFDXGIDeviceManager = manager.ok_or_else(|| Error::from(E_FAIL))?;
        unsafe { manager.ResetDevice(&device, token)? };
        Ok(Self {
            video_device: device.cast()?,
            video_context: context.cast()?,
            device,
            context,
            manager,
        })
    }

    pub fn winrt(&self) -> Result<IDirect3DDevice> {
        let dxgi: IDXGIDevice = self.device.cast()?;
        unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? }.cast()
    }
}
