use super::*;

/// A GPU-resident bitmap.
#[derive(Clone)]
pub struct Bitmap(pub(crate) ID2D1Bitmap1);

impl Bitmap {
    pub(crate) fn load_from_file(
        context: &ID2D1DeviceContext,
        path: &std::path::Path,
    ) -> Result<Self> {
        unsafe {
            let wic_factory = wic_factory()?;

            let wide_path: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let decoder = wic_factory.CreateDecoderFromFilename(
                PCWSTR(wide_path.as_ptr()),
                std::ptr::null(),
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )?;

            let frame = decoder.GetFrame(0)?;
            let converter = wic_factory.CreateFormatConverter()?;

            converter
                .Initialize(
                    &frame,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeMedianCut,
                )
                .ok()?;

            let bitmap = context.CreateBitmapFromWicBitmap(&converter, None)?;
            Ok(Self(bitmap))
        }
    }

    /// Creates a GPU bitmap from a buffer of 32-bit BGRA pixels laid out row by
    /// row with no padding between rows (tightly packed, `width * 4` bytes per
    /// row). `alpha` selects how the alpha channel is interpreted.
    pub(crate) fn from_bytes(
        context: &ID2D1DeviceContext,
        pixels: &[u8],
        width: u32,
        height: u32,
        alpha: AlphaMode,
    ) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        let stride = width
            .checked_mul(4)
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let expected = stride
            .checked_mul(height)
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))? as usize;
        if pixels.len() != expected {
            return Err(Error::from_hresult(E_INVALIDARG));
        }

        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: alpha.to_abi(),
            },
            dpiX: 96.0,
            dpiY: 96.0,
            ..Default::default()
        };
        let size = D2D_SIZE_U { width, height };

        let bitmap = unsafe {
            context.CreateBitmap(size, Some(pixels.as_ptr().cast()), stride, &properties)?
        };
        Ok(Self(bitmap))
    }

    /// Creates a GPU bitmap from BC1 (DXT1)-compressed texel data — 8 bytes
    /// per 4x4 pixel block, tightly packed row-major by block (no padding
    /// between block rows). Direct2D has supported BC1/BC2/BC3 as *source*
    /// pixel formats (never a render target) since Windows 8.1, across all
    /// feature levels, always with premultiplied alpha — see Microsoft's
    /// "Block compression" Direct2D doc. `width`/`height` must each be a
    /// multiple of 4 (the block size); the caller is responsible for padding
    /// its source content to that before compressing, same as any other BC1
    /// producer.
    pub(crate) fn from_bc1(
        context: &ID2D1DeviceContext,
        compressed: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Self> {
        if width == 0 || height == 0 || width % 4 != 0 || height % 4 != 0 {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        let blocks_wide = width / 4;
        let blocks_high = height / 4;
        // 8 bytes/block for BC1 — the "pitch" D2D wants here is the byte
        // size of one row of *blocks*, mirroring what a DDS/D3D11 BC1
        // surface's own row pitch would be, not a per-pixel byte stride.
        let pitch = blocks_wide
            .checked_mul(8)
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let expected = (pitch as usize)
            .checked_mul(blocks_high as usize)
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        if compressed.len() != expected {
            return Err(Error::from_hresult(E_INVALIDARG));
        }

        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_BC1_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            ..Default::default()
        };
        let size = D2D_SIZE_U { width, height };

        let bitmap = unsafe {
            context.CreateBitmap(size, Some(compressed.as_ptr().cast()), pitch, &properties)?
        };
        Ok(Self(bitmap))
    }

    /// Returns the width of the bitmap, in device-independent pixels.
    pub fn width(&self) -> f32 {
        let size = unsafe { self.0.GetSize() };
        size.width
    }

    /// Returns the height of the bitmap, in device-independent pixels.
    pub fn height(&self) -> f32 {
        let size = unsafe { self.0.GetSize() };
        size.height
    }
}

fn wic_factory() -> Result<IWICImagingFactory> {
    thread_local! {
        static SHARED: std::cell::OnceCell<IWICImagingFactory> = const { std::cell::OnceCell::new() };
    }

    SHARED.with(|cell| {
        if let Some(factory) = cell.get() {
            return Ok(factory.clone());
        }

        let factory: IWICImagingFactory = unsafe {
            let mut ptr = core::ptr::null_mut();
            CoCreateInstance(
                &CLSID_WICImagingFactory,
                core::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &<IWICImagingFactory as Interface>::IID,
                &mut ptr,
            )
            .ok()?;
            Type::from_abi(ptr)?
        };

        Ok(cell.get_or_init(|| factory).clone())
    })
}
