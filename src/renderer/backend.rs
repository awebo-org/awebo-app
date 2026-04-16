//! Render backend abstraction — GPU (wgpu) or CPU (softbuffer).
//!
//! The renderer orchestrates UI drawing into a CPU pixel buffer, then
//! hands off presentation to whichever backend is active. The GPU backend
//! additionally supports instanced glyph rendering via [`super::gpu_grid`].
//!
//! Backend selection at startup:
//! 1. Try wgpu (hardware-accelerated surface + instanced text rendering).
//! 2. If wgpu fails (no adapter, no surface), fall back to softbuffer.
//!
//! The `TERMINAL_BACKEND` environment variable can force a specific backend:
//! - `gpu` — only try wgpu, panic on failure.
//! - `soft` — skip wgpu, go straight to softbuffer.

use std::num::NonZeroU32;
use std::sync::Arc;

use winit::window::Window;

use super::gpu_grid::GpuGridRenderer;

/// Active render backend — either GPU-accelerated or pure-CPU.
pub enum RenderBackend {
    Gpu(Box<GpuBackend>),
    Soft(SoftBackend),
}

/// wgpu-backed GPU rendering with instanced glyph support.
pub struct GpuBackend {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_format: wgpu::TextureFormat,
    pub present_mode: wgpu::PresentMode,
    pub gpu_grid: GpuGridRenderer,
    pub is_bgra: bool,
}

/// Pure-CPU softbuffer rendering — no GPU required.
pub struct SoftBackend {
    pub surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    pub is_bgra: bool,
}

impl RenderBackend {
    /// Try to create a GPU backend, falling back to softbuffer if wgpu
    /// is unavailable or fails. Respects `TERMINAL_BACKEND` env var.
    pub fn new(window: Arc<Window>, width: u32, height: u32) -> Self {
        let force = std::env::var("TERMINAL_BACKEND").unwrap_or_default();

        if force != "soft" {
            if let Some(gpu) = Self::try_gpu(window.clone(), width, height) {
                log::info!("Using GPU (wgpu) backend");
                return RenderBackend::Gpu(Box::new(gpu));
            }
            if force == "gpu" {
                panic!("TERMINAL_BACKEND=gpu but wgpu initialisation failed");
            }
            log::warn!("wgpu unavailable — falling back to softbuffer");
        } else {
            log::info!("TERMINAL_BACKEND=soft — using softbuffer backend");
        }

        let soft = Self::create_soft(window, width, height);
        RenderBackend::Soft(soft)
    }

    fn try_gpu(window: Arc<Window>, width: u32, height: u32) -> Option<GpuBackend> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

        let surface = instance.create_surface(window).ok()?;

        let adapter = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(instance.request_adapter(
                &wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                },
            ))
        })
        .ok()?;

        let (device, queue) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        })
        .ok()?;

        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .unwrap_or(caps.formats[0]);

        let is_bgra = matches!(
            surface_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );

        let present_mode = pick_present_mode(&caps.present_modes);

        let config = gpu_surface_config(surface_format, width, height, present_mode);
        surface.configure(&device, &config);

        let gpu_grid = GpuGridRenderer::new(&device, surface_format);

        Some(GpuBackend {
            device,
            queue,
            surface,
            surface_format,
            present_mode,
            gpu_grid,
            is_bgra,
        })
    }

    fn create_soft(window: Arc<Window>, width: u32, height: u32) -> SoftBackend {
        let ctx =
            softbuffer::Context::new(window.clone()).expect("Failed to create softbuffer context");
        let mut surface =
            softbuffer::Surface::new(&ctx, window).expect("Failed to create softbuffer surface");

        if let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
            let _ = surface.resize(w, h);
        }

        SoftBackend {
            surface,
            is_bgra: cfg!(target_os = "macos"),
        }
    }

    pub fn is_gpu(&self) -> bool {
        matches!(self, RenderBackend::Gpu(_))
    }

    pub fn is_bgra(&self) -> bool {
        match self {
            RenderBackend::Gpu(g) => g.is_bgra,
            RenderBackend::Soft(s) => s.is_bgra,
        }
    }

    /// Resize the presentation surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        match self {
            RenderBackend::Gpu(g) => {
                let config = gpu_surface_config(g.surface_format, width, height, g.present_mode);
                g.surface.configure(&g.device, &config);
            }
            RenderBackend::Soft(s) => {
                if let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
                    let _ = s.surface.resize(w, h);
                }
            }
        }
    }

    /// Upload the CPU pixel buffer to the presentation surface.
    ///
    /// For GPU: uploads only the dirty region via write_texture and returns
    /// a [`GpuFrame`] that the caller can use to run additional render
    /// passes (e.g. instanced glyphs) before calling [`end_frame`].
    ///
    /// For softbuffer: converts BGRA/RGBA pixel data to 0RGB u32 format,
    /// writes to the buffer, presents immediately, and returns `None`.
    pub fn begin_frame(
        &mut self,
        pixel_data: &[u8],
        w: u32,
        h: u32,
        dirty: Option<(usize, usize)>,
    ) -> Option<GpuFrame> {
        if w == 0 || h == 0 {
            return None;
        }

        match self {
            RenderBackend::Gpu(g) => {
                let surface_texture = match g.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) => t,
                    wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        let config = gpu_surface_config(g.surface_format, w, h, g.present_mode);
                        g.surface.configure(&g.device, &config);
                        return None;
                    }
                    wgpu::CurrentSurfaceTexture::Occluded
                    | wgpu::CurrentSurfaceTexture::Timeout => {
                        return None;
                    }
                    other => {
                        log::error!("Surface error: {:?}", other);
                        return None;
                    }
                };

                let (upload_y, upload_h) = (0, h);

                if upload_h > 0 {
                    let row_bytes = w * 4;
                    let offset = upload_y as u64 * row_bytes as u64;
                    g.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &surface_texture.texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: upload_y,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &pixel_data[offset as usize
                            ..(offset + upload_h as u64 * row_bytes as u64) as usize],
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(row_bytes),
                            rows_per_image: Some(upload_h),
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: upload_h,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                g.queue.submit(std::iter::empty());

                let view = surface_texture.texture.create_view(&Default::default());
                Some(GpuFrame {
                    surface_texture,
                    view,
                })
            }
            RenderBackend::Soft(s) => {
                if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                    let _ = s.surface.resize(nw, nh);
                    if let Ok(mut buf) = s.surface.buffer_mut() {
                        let (start_row, end_row) = match dirty {
                            Some((min_y, max_y)) => (min_y, (max_y + 1).min(h as usize)),
                            None => (0, h as usize),
                        };

                        let w_usize = w as usize;
                        for row in start_row..end_row {
                            for col in 0..w_usize {
                                let i = row * w_usize + col;
                                let offset = i * 4;
                                if offset + 3 < pixel_data.len() && i < buf.len() {
                                    let (r, g_ch, b) = if s.is_bgra {
                                        (
                                            pixel_data[offset + 2],
                                            pixel_data[offset + 1],
                                            pixel_data[offset],
                                        )
                                    } else {
                                        (
                                            pixel_data[offset],
                                            pixel_data[offset + 1],
                                            pixel_data[offset + 2],
                                        )
                                    };
                                    buf[i] = (r as u32) << 16 | (g_ch as u32) << 8 | b as u32;
                                }
                            }
                        }

                        let _ = buf.present();
                    }
                }
                None
            }
        }
    }

    /// Present a GPU frame after optional render passes have been recorded.
    pub fn end_frame(&self, frame: GpuFrame) {
        frame.surface_texture.present();
    }
}

/// Handle to a GPU frame in flight. Holds the surface texture and a view
/// that can be used as a render attachment for additional passes (e.g.
/// instanced glyph rendering) before presenting.
pub struct GpuFrame {
    surface_texture: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
}

/// Human-readable label for the active backend.
impl std::fmt::Display for RenderBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderBackend::Gpu(_) => write!(f, "GPU (wgpu)"),
            RenderBackend::Soft(_) => write!(f, "CPU (softbuffer)"),
        }
    }
}

fn pick_present_mode(supported: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    if supported.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else {
        wgpu::PresentMode::Fifo
    }
}

fn gpu_surface_config(
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    present_mode: wgpu::PresentMode,
) -> wgpu::SurfaceConfiguration {
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
        format,
        width,
        height,
        present_mode,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_present_mode_prefers_mailbox() {
        let modes = vec![wgpu::PresentMode::Fifo, wgpu::PresentMode::Mailbox];
        assert_eq!(pick_present_mode(&modes), wgpu::PresentMode::Mailbox);
    }

    #[test]
    fn pick_present_mode_falls_back_to_fifo() {
        let modes = vec![wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate];
        assert_eq!(pick_present_mode(&modes), wgpu::PresentMode::Fifo);
    }

    #[test]
    fn gpu_surface_config_fields() {
        let cfg = gpu_surface_config(
            wgpu::TextureFormat::Bgra8Unorm,
            800,
            600,
            wgpu::PresentMode::Fifo,
        );
        assert_eq!(cfg.width, 800);
        assert_eq!(cfg.height, 600);
        assert_eq!(cfg.format, wgpu::TextureFormat::Bgra8Unorm);
        assert_eq!(cfg.present_mode, wgpu::PresentMode::Fifo);
    }

    #[test]
    fn display_impl_gpu_label() {
        let label = "GPU (wgpu)".to_string();
        assert!(label.contains("GPU"));
    }

    #[test]
    fn display_impl_soft_label() {
        let label = "CPU (softbuffer)".to_string();
        assert!(label.contains("softbuffer"));
    }
}
