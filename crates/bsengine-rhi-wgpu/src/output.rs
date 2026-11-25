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
        /// `None` while the app is in the background: Android takes the
        /// window's native surface away when the app leaves the screen and
        /// gives it a new one when it comes back (`Output::release_surface`,
        /// `Output::recreate_surface`).
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
        /// What the surface is made again from on the way back. The device
        /// belongs to this instance: a surface from another instance is one
        /// the device cannot present to.
        instance: wgpu::Instance,
        /// What the new surface's formats are checked against before it is
        /// configured -- configuring one in a format it lacks is a fatal
        /// validation error, not one `configure` returns.
        adapter: wgpu::Adapter,
        window: Arc<winit::window::Window>,
        /// How many surfaces this window has had: 1 from the start, one more
        /// for every return from the background. What the test of that path
        /// reads to know the path ran.
        surfaces_made: u32,
        /// Frames presented to this window, every surface it has had
        /// counted. A frame that found no surface is not one.
        presented: u64,
    },
    /// A texture we own. Nothing is presented, and the pixels can be read back.
    Offscreen {
        texture: crate::profiler::TrackedTexture,
        width: u32,
        height: u32,
        /// [`OFFSCREEN_FORMAT`]; `Rgba16Float` for an scRGB-output test, or
        /// `Rgb10a2Unorm` -- [`crate::hdr_output::PQ_SWAPCHAIN_FORMAT`] -- for an
        /// HDR10 one.
        format: wgpu::TextureFormat,
    },
}

impl Output {
    /// Whether this target carries the HDR10 signal (PQ, BT.2020): a swapchain
    /// configured with that colour space, or the offscreen target an HDR10
    /// test reads -- the one offscreen target in that format.
    pub(crate) fn is_pq(&self) -> bool {
        match self {
            Output::Window { config, .. } => {
                config.color_space == wgpu::SurfaceColorSpace::Bt2100Pq
            }
            Output::Offscreen { format, .. } => *format == crate::hdr_output::PQ_SWAPCHAIN_FORMAT,
        }
    }

    /// The format the post-process chain writes the frame in: the target's
    /// own, except under HDR10, where the frame is scRGB in a texture of its
    /// own and only `hdr_output::PqOutput` writes the target, and for a
    /// target that cannot encode sRGB, where the frame is sRGB in a texture
    /// of its own and only `srgb_encode::SrgbEncode` writes the target.
    pub(crate) fn frame_format(&self) -> wgpu::TextureFormat {
        if self.is_pq() {
            crate::hdr_output::PQ_FRAME_FORMAT
        } else if self.needs_srgb_encode() {
            crate::srgb_encode::ENCODED_FRAME_FORMAT
        } else {
            self.format()
        }
    }

    /// Whether the frame has to be sRGB-encoded by a pass of our own: an
    /// 8-bit target that stores what it is given (`Rgba8Unorm`,
    /// `Bgra8Unorm`) and is not viewed through its sRGB twin. That is an
    /// Android GLES surface, which offers no sRGB format and no other view
    /// format (`WgpuSurface::new`), and the `Rgba8Unorm` offscreen target
    /// the tests stand in for one with. A WebGPU canvas has the same formats
    /// but takes an sRGB view, so `format` is already the twin there; float
    /// and ten-bit targets are HDR output, linear on purpose.
    pub(crate) fn needs_srgb_encode(&self) -> bool {
        matches!(
            self.format(),
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
        )
    }

    /// The format of the target itself.
    pub(crate) fn format(&self) -> wgpu::TextureFormat {
        match self {
            // The sRGB view, where the surface was given one (a WebGPU canvas:
            // see `WgpuSurface::new`).
            Output::Window { config, .. } => config
                .view_formats
                .first()
                .copied()
                .unwrap_or(config.format),
            Output::Offscreen { format, .. } => *format,
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
            Output::Window {
                surface, config, ..
            } => {
                // In the background there is nothing to draw into; the
                // runner draws no frames there, so this is only reached by
                // one already under way.
                let Some(surface) = surface else {
                    return Err("no frame to draw into: the window has no surface \
                                (the app is in the background)"
                        .to_string());
                };
                let frame = match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                    other => return Err(format!("no frame to draw into: {other:?}")),
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                    format: config.view_formats.first().copied(),
                    ..Default::default()
                });
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
                // Kept in `config` when there is no surface: the one made on
                // the way back from the background is configured from it.
                if let Some(surface) = surface {
                    surface.configure(device, config);
                }
            }
            Output::Offscreen {
                texture,
                width: w,
                height: h,
                format,
            } => {
                *texture = create_offscreen_texture_with_format(device, width, height, *format);
                *w = width;
                *h = height;
            }
        }
    }
}

impl Output {
    /// Lets go of the window's surface, for the app going into the
    /// background. Returns whether there was one to let go of (an offscreen
    /// target has none, and a second call finds it gone).
    ///
    /// ⚠️ Has to happen before the runner's `suspended` returns. Android
    /// reclaims the native window as soon as the app has been told, and a
    /// surface left on it is one the next present faults on -- or, when the
    /// app comes back, a second swapchain on the window, which wgpu treats
    /// as fatal (measured on a desktop Vulkan window: a renderer that kept
    /// its surface and made another died in the winit callback,
    /// 0xc000041d). Unity, Unreal and Godot's Android players all destroy the
    /// surface in `surfaceDestroyed` and make a new one in `surfaceCreated`.
    pub(crate) fn release_surface(&mut self) -> bool {
        match self {
            Output::Window { surface, .. } => surface.take().is_some(),
            Output::Offscreen { .. } => false,
        }
    }

    /// Makes the window's surface again, after [`Self::release_surface`], on
    /// the same device and in the same format, at the window's size now.
    /// `Ok(false)` where there is nothing to do: a surface already there, or
    /// an offscreen target.
    ///
    /// The format is the one chosen at start and is checked, not assumed:
    /// a new surface could in principle offer others, and configuring one
    /// in a format it lacks is fatal. A failure here leaves the window
    /// without a surface -- frames then report it and draw nothing -- rather
    /// than ending the game.
    pub(crate) fn recreate_surface(&mut self, device: &wgpu::Device) -> Result<bool, String> {
        let Output::Window {
            surface,
            config,
            instance,
            adapter,
            window,
            surfaces_made,
            ..
        } = self
        else {
            return Ok(false);
        };
        if surface.is_some() {
            return Ok(false);
        }
        let new = instance
            .create_surface(window.clone())
            .map_err(|e| format!("cannot make the window's surface again: {e}"))?;
        let caps = new.get_capabilities(adapter);
        if !caps.formats.contains(&config.format) {
            return Err(format!(
                "the window's new surface does not offer {:?}, the format the renderer \
                 was built for (it offers {:?})",
                config.format, caps.formats
            ));
        }
        // The window may have changed size while it was away (rotated in the
        // background); a zero size is a window not laid out yet, which keeps
        // the last one rather than configuring an empty swapchain.
        let size = window.inner_size();
        if size.width > 0 && size.height > 0 {
            config.width = size.width;
            config.height = size.height;
        }
        new.configure(device, config);
        *surface = Some(new);
        *surfaces_made += 1;
        Ok(true)
    }

    /// Counts a frame presented to the window. Nothing for an offscreen
    /// target, which presents nothing.
    pub(crate) fn note_presented(&mut self) {
        if let Output::Window { presented, .. } = self {
            *presented += 1;
        }
    }

    /// `(surfaces made, frames presented)` for a window; `None` offscreen.
    pub(crate) fn window_counts(&self) -> Option<(u32, u64)> {
        match self {
            Output::Window {
                surfaces_made,
                presented,
                ..
            } => Some((*surfaces_made, *presented)),
            Output::Offscreen { .. } => None,
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
    read_texels(device, queue, texture, width, height, 4)
}

/// [`read_pixels`] for any texel size: the bytes of every texel, tightly
/// packed, top row first. 8 for an `Rgba16Float` target.
pub(crate) fn read_texels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    bytes_per_texel: u32,
) -> Vec<u8> {
    let unpadded_bytes_per_row = width * bytes_per_texel;
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
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
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
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv()
        .expect("map_async never reported a result")
        .expect("mapping the readback buffer failed");

    let mapped = slice.get_mapped_range().expect("a mapped buffer");
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
) -> crate::profiler::TrackedTexture {
    create_offscreen_texture_with_format(device, width, height, OFFSCREEN_FORMAT)
}

/// [`create_offscreen_texture`] in `format`.
pub(crate) fn create_offscreen_texture_with_format(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> crate::profiler::TrackedTexture {
    crate::profiler::create_tracked_texture(
        device,
        &wgpu::TextureDescriptor {
            label: Some("offscreen output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        },
    )
}
