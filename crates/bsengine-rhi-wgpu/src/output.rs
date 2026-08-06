//! Where a rendered frame goes, and how to get it back.
//!
//! `WgpuSurface` used to hold a `wgpu::Surface`, its `SurfaceConfiguration` and
//! the window handle as three separate fields, which is why it could not exist
//! without a window. The window's only real use was `create_surface` -- the
//! handle field was already `_window`, held to keep the window alive and never
//! read.
//!
//! One enum owns all three now. The window path behaves exactly as before; the
//! offscreen path renders into a texture we own, skips presenting, and can hand
//! the pixels back.

use std::sync::Arc;

/// The format of an offscreen target.
///
/// The window path picks an sRGB format out of the surface's capabilities, but
/// an offscreen target has no capabilities to ask. The composite pipeline
/// expects sRGB, so it is pinned here.
pub(crate) const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The target `WgpuSurface` composites its final image into.
pub(crate) enum Output {
    /// A window's swapchain. The frame is presented and cannot be read back: a
    /// swapchain texture is not created with `COPY_SRC`.
    Window {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        _window: Arc<winit::window::Window>,
    },
    /// A texture we own. Nothing is presented, and the pixels can be read back.
    Offscreen {
        texture: wgpu::Texture,
        width: u32,
        height: u32,
    },
}

impl Output {
    /// The format the final composite writes in.
    pub(crate) fn format(&self) -> wgpu::TextureFormat {
        match self {
            Output::Window { config, .. } => config.format,
            Output::Offscreen { .. } => OFFSCREEN_FORMAT,
        }
    }

    /// Width of the target, in pixels.
    pub(crate) fn width(&self) -> u32 {
        match self {
            Output::Window { config, .. } => config.width,
            Output::Offscreen { width, .. } => *width,
        }
    }

    /// Height of the target, in pixels.
    pub(crate) fn height(&self) -> u32 {
        match self {
            Output::Window { config, .. } => config.height,
            Output::Offscreen { height, .. } => *height,
        }
    }

    /// The view to render this frame into, plus the swapchain frame to present
    /// once it has been submitted.
    ///
    /// Offscreen there is nothing to present, so the second half is `None`.
    /// Handing both back together is safe: a `TextureView` reference-counts the
    /// texture it was made from, so the caller can hold it alongside the
    /// `SurfaceTexture` exactly as the original inline code did.
    pub(crate) fn acquire(
        &self,
    ) -> Result<(wgpu::TextureView, Option<wgpu::SurfaceTexture>), String> {
        match self {
            Output::Window { surface, .. } => {
                let frame = surface.get_current_texture().map_err(|e| e.to_string())?;
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                Ok((view, Some(frame)))
            }
            Output::Offscreen { texture, .. } => {
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                Ok((view, None))
            }
        }
    }

    /// Resizes the target. The window path reconfigures its swapchain; the
    /// offscreen path allocates a new texture, since a texture's size is fixed
    /// at creation.
    pub(crate) fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        match self {
            Output::Window {
                surface, config, ..
            } => {
                config.width = width;
                config.height = height;
                surface.configure(device, config);
            }
            Output::Offscreen {
                texture,
                width: w,
                height: h,
            } => {
                *texture = create_offscreen_texture(device, width, height);
                *w = width;
                *h = height;
            }
        }
    }
}

/// Reads a texture's pixels back as tightly packed RGBA8.
///
/// `copy_texture_to_buffer` requires each row to start on a 256-byte boundary,
/// so the copy goes through a padded buffer and is unpadded row by row.
///
/// Code that skips that padding still works whenever `width * 4` divides
/// evenly by 256 -- that is, whenever the width is a multiple of 64. It is why
/// the tests render at 200x150 rather than at a rounder size: `200 * 4 = 800`,
/// and `800 % 256 = 32`.
pub(crate) fn read_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let unpadded_bytes_per_row = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (padded_bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("readback encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .expect("map_async never reported a result")
        .expect("mapping the readback buffer failed");

    let mapped = slice.get_mapped_range();
    let mut tight = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + unpadded_bytes_per_row as usize;
        tight.extend_from_slice(&mapped[start..end]);
    }
    drop(mapped);
    buffer.unmap();
    tight
}

/// A texture that is both a render target and a copy source. `COPY_SRC` is what
/// makes reading the frame back possible at all.
pub(crate) fn create_offscreen_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen output"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}
