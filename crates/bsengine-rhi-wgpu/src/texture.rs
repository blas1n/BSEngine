use bsengine_core::{TextureFilter, TextureImportSettings, TextureWrap};
use bsengine_ecs::Resource;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct GpuTexture {
    _texture: crate::profiler::TrackedTexture,
    _view: wgpu::TextureView,
    /// Per texture, because filter and wrap are import settings: two
    /// textures on screen at once can legitimately want different ones, and
    /// one registry-wide sampler could only ever honour one of them.
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    settings: TextureImportSettings,
    /// The format the object is in, decided once at upload by
    /// [`GpuTextureRegistry::upload_format`] and kept, so that a rebuild for
    /// streaming copies levels between two objects of one format. It is not
    /// a function of `settings` alone: a requested compression is refused
    /// on a device without the feature and on an image that is not whole
    /// blocks, and re-deciding at each rebuild would have to repeat both
    /// checks -- and get them the same -- or copy BC1 blocks into RGBA8.
    format: wgpu::TextureFormat,
    /// Present for a texture whose mips are streamed; see [`Streamed`].
    streamed: Option<Streamed>,
    /// Which GPU object this is, counted registry-wide: a new number every
    /// time the object behind an id is rebuilt -- a hot reload, a streamed
    /// level in or out. A consumer holding its own copy of the object (the
    /// skybox) compares this to know when its copy is stale, which no field
    /// of the pixels can tell it: a reload can bring identical dimensions.
    generation: u64,
}

/// The streaming side of a texture: the whole mip chain, kept on the CPU so
/// levels can be brought in and dropped, and which of them the GPU currently
/// holds.
///
/// # Residency is a texture object, not a flag
///
/// A `wgpu::Texture` allocates every mip level it is declared with, so a
/// texture "with only its small mips resident" cannot be one object with
/// some levels left unwritten -- the memory would be spent regardless, and
/// the point of streaming is the memory. Unreal reallocates a streamed
/// texture at each residency change and this does the same: the GPU object
/// is exactly as large as the resident levels, and raising or lowering
/// residency builds a new one. `resident_base` is the index into `levels` of
/// the largest resident level; `0` is fully resident.
///
/// # Where the levels that are not resident live
///
/// Unity keeps a texture's mips in its imported Library copy, Unreal in the
/// cooked `.ubulk`, Godot in the `.ctex` under `.godot/imported/`: a file
/// with every level laid out for reading one at a time, made once at
/// import, so that streaming a level in is a read and not a decode. This
/// does the same with a **mip cache file** under the registry's cache
/// directory ([`GpuTextureRegistry::set_mip_cache_root`]), written the first
/// time a streamed texture with those pixels is uploaded and keyed by a
/// hash of the pixels, so a re-imported image gets a new file and two
/// identical images share one. With the file in place the registry keeps
/// **no copy of the chain in RAM**: every rebuild reads the levels it
/// uploads from the file. Without a cache directory, or when the file
/// cannot be written (a read-only install), the chain stays in RAM as it
/// did before the cache existed -- streaming still works, it just costs the
/// memory.
struct Streamed {
    source: MipSource,
    /// `(width, height)` of every level, level 0 first, down to 1x1.
    dims: Vec<(u32, u32)>,
    resident_base: u32,
    /// The deepest chain index `resident_base` may sit at. The last level
    /// for an uncompressed texture; for a block-compressed one the last
    /// level of the run from level 0 whose sides are all multiples of 4,
    /// because the object's level 0 must be whole blocks -- wgpu refuses to
    /// create a 2x2 BC1 texture (`NotMultipleOfBlockWidth`), while a 2x2
    /// *mip* of a larger object is fine, padded to its block. So a
    /// compressed 64x64 chain streams between 64 and 4, holding 2x2 and 1x1
    /// as mips of whatever object is current, and never becomes an object
    /// of its own at those sizes. See [`residency_floor`].
    floor: u32,
    /// The chain index the texture *ought* to have resident, from the
    /// largest it is drawn on screen: what [`GpuTextureRegistry::set_wants`]
    /// records each frame and [`GpuTextureRegistry::step_streaming`] moves
    /// `resident_base` towards. `0` -- whole -- until something says
    /// otherwise, so a texture nothing on screen uses (a UI image, a
    /// particle sheet) streams to full as it did before wants existed.
    wanted_base: u32,
    /// The level a worker thread is reading out of the cache file for this
    /// texture, if one is; see [`PendingRead`]. A texture with a read in
    /// flight is left alone by [`GpuTextureRegistry::step_streaming`] until
    /// it lands, so the level arrives for the residency it was read for.
    pending: Option<PendingRead>,
}

/// One level of a streamed texture, being read from its cache file on a
/// worker thread.
///
/// The read is off the frame thread because that is the whole point of a
/// cache file: a level's bytes come from disk, and a disk -- a laptop's, a
/// network drive's, anything but a warm SSD -- can take longer than a frame
/// to hand them over. Unreal issues an asynchronous IO request per mip and
/// uploads on the frame it arrives; Unity's streaming loads in the
/// background and applies the result. This is that: [`GpuTextureRegistry::raise_residency`]
/// starts the read and returns, and [`GpuTextureRegistry::poll_reads`], once
/// a frame, uploads whatever has arrived.
///
/// A thread per read rather than a pool: a raise is one level, at most a
/// few per second across every streamed texture, and the thread's whole job
/// is one `read_exact`.
struct PendingRead {
    /// The chain index the read is for -- the new resident base.
    base: u32,
    /// Filled once by the worker; `None` until then. A slot rather than a
    /// channel because the registry is a resource and a `Receiver` is not
    /// `Sync`.
    result: Arc<std::sync::Mutex<Option<std::io::Result<Vec<u8>>>>>,
}

impl PendingRead {
    /// Starts reading `len` bytes at `offset` of `path` -- one level of the
    /// cache file -- on a worker thread.
    fn start(path: PathBuf, offset: u64, len: u64, base: u32) -> Self {
        let result = Arc::new(std::sync::Mutex::new(None));
        let slot = Arc::clone(&result);
        std::thread::spawn(move || {
            let read = read_level_from_file(&path, offset, len);
            *slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(read);
        });
        Self { base, result }
    }

    /// The read's outcome, once the worker has written it.
    fn take(&self) -> Option<std::io::Result<Vec<u8>>> {
        self.result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

/// Reads one level's `len` bytes at `offset` of the cache file at `path`.
///
/// # Errors
///
/// The file is gone or short.
fn read_level_from_file(path: &Path, offset: u64, len: u64) -> std::io::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let mut bytes = vec![0u8; len as usize];
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// Where a streamed texture's levels are read from at each rebuild.
enum MipSource {
    /// The whole chain in RAM, level 0 first.
    Memory(Vec<(u32, u32, Vec<u8>)>),
    /// A mip cache file; `table[i]` is level `i`'s `(offset, len)` in it.
    Disk {
        path: PathBuf,
        table: Vec<(u64, u64)>,
    },
}

/// The first bytes of a mip cache file. The digit is the layout version:
/// a reader that finds another rewrites the file rather than trusting it.
/// `02` added the encoding tag after the level count, when the file began
/// to hold block-compressed levels as well as RGBA8 ones.
const MIP_CACHE_MAGIC: &[u8; 8] = b"BSMIPS02";

impl Streamed {
    /// The levels from `base` down to the smallest, read whole -- what a
    /// raise used to do on the frame thread, kept for tests that check what
    /// a cache file holds. The registry itself now reads one level at a
    /// time, off the frame thread (see [`PendingRead`]).
    ///
    /// # Errors
    ///
    /// The cache file is gone or short.
    #[cfg(test)]
    fn levels_from(
        &self,
        base: u32,
    ) -> std::io::Result<Vec<(u32, u32, std::borrow::Cow<'_, [u8]>)>> {
        match &self.source {
            MipSource::Memory(levels) => Ok(levels[base as usize..]
                .iter()
                .map(|(w, h, p)| (*w, *h, std::borrow::Cow::Borrowed(p.as_slice())))
                .collect()),
            MipSource::Disk { path, table } => {
                let mut out = Vec::with_capacity(self.dims.len() - base as usize);
                for (i, (w, h)) in self.dims.iter().enumerate().skip(base as usize) {
                    let (offset, len) = table[i];
                    let bytes = read_level_from_file(path, offset, len)?;
                    out.push((*w, *h, std::borrow::Cow::Owned(bytes)));
                }
                Ok(out)
            }
        }
    }

    /// Bytes of the chain this texture holds in RAM: the whole chain from
    /// memory, nothing from a cache file. What makes the cache's saving
    /// observable without trusting it.
    fn ram_bytes(&self) -> u64 {
        match &self.source {
            MipSource::Memory(levels) => levels.iter().map(|(_, _, p)| p.len() as u64).sum(),
            MipSource::Disk { .. } => 0,
        }
    }
}

/// Serialises a chain as a mip cache file: the magic, the level count, the
/// encoding tag ([`encoding_tag`]), then per level `(width, height, offset,
/// len)` as little-endian `u32, u32, u64, u64`, then the levels' bytes back
/// to back -- RGBA8 or the format's blocks. Returns the bytes and the table
/// [`MipSource::Disk`] reads by.
fn encode_mip_cache(
    levels: &[(u32, u32, std::borrow::Cow<'_, [u8]>)],
    format: wgpu::TextureFormat,
) -> (Vec<u8>, Vec<(u64, u64)>) {
    let header_len = MIP_CACHE_HEADER_LEN + levels.len() * 24;
    let mut out = Vec::with_capacity(header_len + levels.iter().map(|l| l.2.len()).sum::<usize>());
    out.extend_from_slice(MIP_CACHE_MAGIC);
    out.extend_from_slice(&(levels.len() as u32).to_le_bytes());
    out.extend_from_slice(&encoding_tag(format).to_le_bytes());
    let mut table = Vec::with_capacity(levels.len());
    let mut offset = header_len as u64;
    for (w, h, pixels) in levels {
        let len = pixels.len() as u64;
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&h.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        table.push((offset, len));
        offset += len;
    }
    for (_, _, pixels) in levels {
        out.extend_from_slice(pixels);
    }
    (out, table)
}

/// Magic, level count, encoding tag.
const MIP_CACHE_HEADER_LEN: usize = 8 + 4 + 4;

/// Reads a mip cache file's header and checks it describes exactly `dims`
/// in `format`'s encoding with every level's bytes present. `None` for a
/// file of another version, another chain, another encoding, or one cut
/// short -- which is rewritten, not trusted.
fn read_mip_cache_table(
    path: &Path,
    dims: &[(u32, u32)],
    format: wgpu::TextureFormat,
) -> Option<Vec<(u64, u64)>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = vec![0u8; MIP_CACHE_HEADER_LEN + dims.len() * 24];
    file.read_exact(&mut header).ok()?;
    parse_mip_cache_table(&header, file.metadata().ok()?.len(), dims, format)
}

/// The checks [`read_mip_cache_table`] makes, on a header already in hand:
/// from a file, or from bytes a package shipped (see
/// [`GpuTextureRegistry::set_shipped_mip_cache`]). `file_len` is the whole
/// file's length, so a table pointing past its end is refused.
fn parse_mip_cache_table(
    header: &[u8],
    file_len: u64,
    dims: &[(u32, u32)],
    format: wgpu::TextureFormat,
) -> Option<Vec<(u64, u64)>> {
    if header.len() < MIP_CACHE_HEADER_LEN + dims.len() * 24 {
        return None;
    }
    if &header[..8] != MIP_CACHE_MAGIC {
        return None;
    }
    let count = u32::from_le_bytes(header[8..12].try_into().ok()?) as usize;
    if count != dims.len() {
        return None;
    }
    let tag = u32::from_le_bytes(header[12..16].try_into().ok()?);
    if tag != encoding_tag(format) {
        return None;
    }
    let mut table = Vec::with_capacity(count);
    for (i, (w, h)) in dims.iter().enumerate() {
        let at = MIP_CACHE_HEADER_LEN + i * 24;
        let fw = u32::from_le_bytes(header[at..at + 4].try_into().ok()?);
        let fh = u32::from_le_bytes(header[at + 4..at + 8].try_into().ok()?);
        let offset = u64::from_le_bytes(header[at + 8..at + 16].try_into().ok()?);
        let len = u64::from_le_bytes(header[at + 16..at + 24].try_into().ok()?);
        if (fw, fh) != (*w, *h)
            || len != crate::profiler::level_bytes(format, *w, *h)
            || offset + len > file_len
        {
            return None;
        }
        table.push((offset, len));
    }
    Some(table)
}

/// Every level of a cache file already in memory -- the copy a package
/// shipped -- or `None` if it does not describe `dims` in `format`'s
/// encoding.
fn levels_from_bytes(
    bytes: &[u8],
    dims: &[(u32, u32)],
    format: wgpu::TextureFormat,
) -> Option<Vec<(u32, u32, std::borrow::Cow<'static, [u8]>)>> {
    let table = parse_mip_cache_table(bytes, bytes.len() as u64, dims, format)?;
    Some(
        dims.iter()
            .zip(&table)
            .map(|((w, h), (offset, len))| {
                let range = *offset as usize..(*offset + *len) as usize;
                (*w, *h, std::borrow::Cow::Owned(bytes[range].to_vec()))
            })
            .collect(),
    )
}

/// Every level of a cache file, read whole -- what an unstreamed
/// compressed texture does at load instead of encoding again.
///
/// # Errors
///
/// The file is gone or short.
fn read_all_levels(
    path: &Path,
    dims: &[(u32, u32)],
    table: &[(u64, u64)],
) -> std::io::Result<Vec<(u32, u32, std::borrow::Cow<'static, [u8]>)>> {
    dims.iter()
        .zip(table)
        .map(|((w, h), (offset, len))| {
            read_level_from_file(path, *offset, *len)
                .map(|bytes| (*w, *h, std::borrow::Cow::Owned(bytes)))
        })
        .collect()
}

/// The GPU format a texture's settings ask for, on a device that has
/// (`bc_supported`) or lacks block compression -- lacking it, the texture
/// goes up as RGBA8 and looks the same at eight times the memory. The
/// `…Srgb` variants are what make `srgb` mean anything: such a view decodes
/// on sample, so the shader receives linear light without a `pow` of its
/// own. Uploaded bytes are identical either way.
fn format_for(settings: TextureImportSettings) -> wgpu::TextureFormat {
    use bsengine_core::TextureCompression;
    match (settings.compression, settings.srgb) {
        (TextureCompression::None, true) => wgpu::TextureFormat::Rgba8UnormSrgb,
        (TextureCompression::None, false) => wgpu::TextureFormat::Rgba8Unorm,
        (TextureCompression::Bc1, true) => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        (TextureCompression::Bc1, false) => wgpu::TextureFormat::Bc1RgbaUnorm,
        (TextureCompression::Bc3, true) => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        (TextureCompression::Bc3, false) => wgpu::TextureFormat::Bc3RgbaUnorm,
    }
}

/// The block encoder for a format, `None` for an uncompressed one.
fn block_encoder(format: wgpu::TextureFormat) -> Option<texpresso::Format> {
    match format {
        wgpu::TextureFormat::Bc1RgbaUnorm | wgpu::TextureFormat::Bc1RgbaUnormSrgb => {
            Some(texpresso::Format::Bc1)
        }
        wgpu::TextureFormat::Bc3RgbaUnorm | wgpu::TextureFormat::Bc3RgbaUnormSrgb => {
            Some(texpresso::Format::Bc3)
        }
        _ => None,
    }
}

/// What a cache file records about how its levels are encoded, so a file
/// written for one encoding is never read as another. Today the level
/// lengths already tell the three apart -- a block-aligned level is 64ab
/// bytes in RGBA8, 16ab in BC3 and 8ab in BC1 -- and the file name's hash
/// carries the tag too, so the check is a third guard; it is here for the
/// encoding whose lengths coincide with one of these (BC7 is 16 bytes a
/// block, exactly BC3's), so that adding it cannot read a BC3 file as BC7.
/// sRGB is not part of it -- the bytes are the same either way and only the
/// view differs.
fn encoding_tag(format: wgpu::TextureFormat) -> u32 {
    match block_encoder(format) {
        None => 0,
        Some(texpresso::Format::Bc1) => 1,
        Some(texpresso::Format::Bc3) => 3,
        Some(_) => unreachable!("only BC1 and BC3 are produced"),
    }
}

/// One RGBA8 level encoded into `format`'s blocks, or the level itself for
/// an uncompressed format.
///
/// The encoder is texpresso's cluster fit -- the quality setting, not the
/// fast range fit -- parallel over block rows. Quality wins because the
/// result is cached to disk and paid once per image; the range fit's
/// banding would be paid on every frame the texture is looked at.
fn encode_level<'a>(
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    rgba: &'a [u8],
) -> std::borrow::Cow<'a, [u8]> {
    let Some(encoder) = block_encoder(format) else {
        return std::borrow::Cow::Borrowed(rgba);
    };
    let mut out = vec![0u8; encoder.compressed_size(width as usize, height as usize)];
    encoder.compress(
        rgba,
        width as usize,
        height as usize,
        texpresso::Params::default(),
        &mut out,
    );
    std::borrow::Cow::Owned(out)
}

/// Bytes per row of one level as `write_texture` wants it: texels times
/// four, or blocks across times bytes per block.
fn bytes_per_row_for(format: wgpu::TextureFormat, width: u32) -> u32 {
    match block_encoder(format) {
        Some(encoder) => width.div_ceil(4) * encoder.block_size() as u32,
        None => 4 * width,
    }
}

/// The extent a copy of one `width` x `height` level is issued with: the
/// level itself, or for a block format the whole blocks that cover it.
/// wgpu validates a copy in blocks, and a mip level below 4x4 -- 2x2, 1x1,
/// the tail of every chain -- is one block with padding, so its copy is the
/// block; issued at the level's own size it fails with `Copy width is not
/// a multiple of block width` at the first compressed chain uploaded.
fn copy_extent_for(format: wgpu::TextureFormat, width: u32, height: u32) -> wgpu::Extent3d {
    let (block_width, block_height) = format.block_dimensions();
    wgpu::Extent3d {
        width: width.div_ceil(block_width) * block_width,
        height: height.div_ceil(block_height) * block_height,
        depth_or_array_layers: 1,
    }
}

/// The deepest chain index a streamed texture's residency may reach; see
/// [`Streamed::floor`]. For a block format, the last of the run of levels
/// from level 0 that are whole blocks; level 0 itself always is, because
/// [`GpuTextureRegistry::upload_format`] refuses compression otherwise.
fn residency_floor(dims: &[(u32, u32)], format: wgpu::TextureFormat) -> u32 {
    let (block_width, block_height) = format.block_dimensions();
    dims.iter()
        .take_while(|(w, h)| w % block_width == 0 && h % block_height == 0)
        .count()
        .saturating_sub(1) as u32
}

/// The cache file name for a chain: a hash of level 0's pixels, its size
/// and the encoding, so the file follows the image's content and not its
/// path -- a re-imported image lands in a new file, two identical images in
/// one, and the same image compressed two ways in two.
fn mip_cache_name(width: u32, height: u32, rgba: &[u8], format: wgpu::TextureFormat) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&width.to_le_bytes());
    hasher.update(&height.to_le_bytes());
    hasher.update(&encoding_tag(format).to_le_bytes());
    hasher.update(rgba);
    format!("{}.mips", hasher.finalize().to_hex())
}

/// The mip cache file a packaged game needs for one texture, made ahead of
/// time: `(file name, file bytes)`, the same name and bytes the registry
/// would write the first time it uploaded this image with these settings.
/// `None` for a texture that is not block-compressed, or one the registry
/// would upload uncompressed anyway (a side not a multiple of 4), or bytes
/// that do not decode -- the packager then ships nothing extra for it and
/// the runtime does what it always did.
///
/// For the packager (`bsengine_asset::cook::package_with_precook`), which
/// must not depend on this GPU crate; the runtime's `--package` passes this
/// function in. Decodes as the texture loader does (`load_from_memory` then
/// RGBA8) -- the file name is a hash of the decoded pixels, so a different
/// decode would ship a file no runtime ever asks for. Assumes the player's
/// device has BC support, as every desktop GPU does; one without it falls
/// back to RGBA8 and never looks the file up.
pub fn precook_mip_cache(
    file_bytes: &[u8],
    settings: TextureImportSettings,
) -> Option<(String, Vec<u8>)> {
    if settings.compression == bsengine_core::TextureCompression::None {
        return None;
    }
    let image = image::load_from_memory(file_bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    if !width.is_multiple_of(4) || !height.is_multiple_of(4) {
        return None;
    }
    let rgba = image.as_raw();
    let raw = if settings.mipmaps {
        mip_chain(width, height, rgba)
    } else {
        vec![(width, height, std::borrow::Cow::Borrowed(rgba.as_slice()))]
    };
    let format = format_for(settings);
    let levels: Vec<(u32, u32, std::borrow::Cow<'_, [u8]>)> = raw
        .iter()
        .map(|(w, h, pixels)| {
            (
                *w,
                *h,
                std::borrow::Cow::Owned(encode_level(format, *w, *h, pixels).into_owned()),
            )
        })
        .collect();
    let (bytes, _) = encode_mip_cache(&levels, format);
    Some((mip_cache_name(width, height, rgba, format), bytes))
}

/// What one [`GpuTextureRegistry::step_streaming`] call did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingStep {
    /// Brought the next larger level of this texture in.
    Raised(u64),
    /// Dropped the largest resident level of this texture.
    Lowered(u64),
}

/// The chain index of the smallest level that is at least `pixels` across,
/// then `bias` levels smaller, clamped to the chain. `pixels` is how large
/// the texture is drawn on screen; a level that size has one texel per
/// pixel, which is where more texels stop being visible.
///
/// The levels' `(width, height)` are level 0 first, halving. A non-finite
/// or huge `pixels` wants level 0; zero or negative wants the last level.
fn wanted_base_for(dims: &[(u32, u32)], pixels: f32, bias: i32) -> u32 {
    let last = dims.len().saturating_sub(1) as i64;
    // The last index whose larger dimension still covers `pixels`.
    let base = dims
        .iter()
        .rposition(|(w, h)| (*w).max(*h) as f32 >= pixels)
        .unwrap_or(0) as i64;
    (base + bias as i64).clamp(0, last) as u32
}

/// Largest dimension of the levels a streamed texture starts with: enough to
/// draw a recognisable stand-in immediately, small enough that a scene full of
/// streamed textures costs almost nothing to bring up. Unity's streaming
/// starts from its "minimum mip" the same way.
pub const STREAMING_INITIAL_MAX_DIM: u32 = 64;

/// Owns every GPU texture uploaded for the running app, keyed by a registry-assigned id.
#[derive(Resource)]
pub struct GpuTextureRegistry {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    bgl: wgpu::BindGroupLayout,
    textures: HashMap<u64, GpuTexture>,
    next_id: u64,
    /// The next [`GpuTexture::generation`]; see there.
    next_generation: u64,
    /// The streamed texture `raise_next_pending` last brought a level in
    /// on, so the next call moves on to another one.
    last_raised: u64,
    /// How many levels have been read out of cache files, and how many
    /// bytes, counted as each read lands. What makes "lowering never touches
    /// the disk" and "a raise reads one level and no more" observable
    /// without trusting the code that does it.
    disk_reads: u64,
    disk_read_bytes: u64,
    /// Where streamed textures' mip cache files go; `None` keeps every
    /// chain in RAM. See [`Streamed`].
    mip_cache_root: Option<PathBuf>,
    /// Whether the cache directory has already been reported unusable, so
    /// a read-only install logs it once rather than per texture.
    mip_cache_warned: bool,
    /// Whether the device has `TEXTURE_COMPRESSION_BC`. Without it a
    /// texture whose sidecar asks for BC1/BC3 uploads as RGBA8 instead --
    /// said once (`compression_warned`), since it is a property of the
    /// machine and not of any one texture.
    bc_supported: bool,
    compression_warned: bool,
    /// How many chains have been block-encoded, as opposed to read back
    /// from the mip cache. What makes "encoded once, then cached"
    /// observable.
    encodes: u64,
    /// Cache files a package carries, looked up by file name before any
    /// chain is encoded; see [`Self::set_shipped_mip_cache`].
    shipped_mip_cache: Option<ShippedMipCache>,
}

/// Looks a mip cache file up by name in what a package shipped -- a pak
/// archive's `.bsengine_cache/mips/` entries -- and returns its bytes.
pub type ShippedMipCache = Arc<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

/// Where a package keeps the mip cache files its packager made ahead of
/// time (`precook_mip_cache`): a directory of a loose package, an entry
/// prefix inside a pak archive. Not the writable cache directory
/// (`.bsengine_cache/mips`) on purpose -- that one is swept of files unused
/// for two weeks at startup, and a player who has not played in two weeks
/// must not lose what shipped with the game and encode it all again.
pub const SHIPPED_MIP_DIR: &str = ".bsengine_shipped/mips";

impl GpuTextureRegistry {
    /// Creates an empty registry bound to the given wgpu device/queue.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let bgl = Self::create_bgl(&device);
        let bc_supported = device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        Self {
            device,
            queue,
            bgl,
            textures: HashMap::new(),
            next_id: 1,
            next_generation: 1,
            last_raised: 0,
            disk_reads: 0,
            disk_read_bytes: 0,
            mip_cache_root: None,
            mip_cache_warned: false,
            bc_supported,
            compression_warned: false,
            encodes: 0,
            shipped_mip_cache: None,
        }
    }

    /// Whether textures asking for BC1/BC3 actually go up compressed on this
    /// device.
    pub fn bc_supported(&self) -> bool {
        self.bc_supported
    }

    /// How many chains have been block-encoded so far; one that came back
    /// from the mip cache does not count.
    pub fn encodes(&self) -> u64 {
        self.encodes
    }

    /// Makes this registry behave as on a device without block compression,
    /// so the fallback can be tested on one that has it.
    #[cfg(test)]
    pub(crate) fn pretend_bc_unsupported_for_testing(&mut self) {
        self.bc_supported = false;
    }

    /// The format a `width` x `height` texture goes up in on this device.
    /// A requested compression is refused, and the texture goes up as RGBA8
    /// looking the same, in two cases: the device lacks the feature (said
    /// once, since every texture in the project would say it), and the
    /// image is not a multiple of 4 on each side (said per image, since it
    /// is that image's problem to fix). The second is the rule every BC
    /// consumer has -- a block-compressed texture's level 0 is whole blocks
    /// -- and wgpu enforces it at `create_texture`; Unity refuses to
    /// compress such an image with the same warning, rather than padding
    /// it to a size the artist did not draw.
    fn upload_format(
        &mut self,
        mut settings: TextureImportSettings,
        width: u32,
        height: u32,
    ) -> wgpu::TextureFormat {
        use bsengine_core::TextureCompression;
        if settings.compression == TextureCompression::None {
            return format_for(settings);
        }
        if !self.bc_supported {
            if !self.compression_warned {
                self.compression_warned = true;
                tracing::warn!(
                    "[texture] this device has no block-compressed texture support; textures \
                     asking for {:?} upload uncompressed",
                    settings.compression
                );
            }
            settings.compression = TextureCompression::None;
        } else if !width.is_multiple_of(4) || !height.is_multiple_of(4) {
            tracing::warn!(
                "[texture] a {width}x{height} texture asks for {:?}, but a block-compressed \
                 texture must be a multiple of 4 on each side; it uploads uncompressed",
                settings.compression
            );
            settings.compression = TextureCompression::None;
        }
        format_for(settings)
    }

    /// Sets the directory streamed textures' mip cache files are written
    /// to and read from -- the project's `.bsengine_cache/mips`, beside the
    /// Asset Browser's thumbnail cache, for an app; a scratch directory for
    /// a test. `None` (the default) keeps every chain in RAM. Textures
    /// already uploaded keep the source they have.
    pub fn set_mip_cache_root(&mut self, root: Option<PathBuf>) {
        self.mip_cache_root = root;
        self.mip_cache_warned = false;
    }

    /// Where to look for cache files a package shipped, before encoding a
    /// block-compressed chain. The reference engines never compress on the
    /// player's machine -- Unity builds compressed data into its bundles,
    /// Unreal cooks it into the pak, Godot imports to `.ctex` -- and neither
    /// does a packaged BSEngine game: the packager writes each compressed
    /// texture's cache file (`precook_mip_cache`) into [`SHIPPED_MIP_DIR`]
    /// of a loose package or under that prefix inside an archive, and this
    /// lookup finds it -- `WgpuRHIPlugin` installs one for the directory, and
    /// the runtime one for its archive. Before this, every player paid the
    /// encode on first run -- about half a second per 2048² texture -- and a
    /// read-only install paid it on every run, since the cache it wrote to
    /// could not be written.
    pub fn set_shipped_mip_cache(&mut self, lookup: Option<ShippedMipCache>) {
        self.shipped_mip_cache = lookup;
    }

    /// The mip cache directory, if one is set.
    pub fn mip_cache_root(&self) -> Option<&Path> {
        self.mip_cache_root.as_deref()
    }

    /// Bytes of mip chain a streamed texture holds in RAM: its whole chain
    /// when it streams from memory, none when it streams from its cache
    /// file. `None` for a texture that is not streamed.
    pub fn chain_ram_bytes(&self, id: u64) -> Option<u64> {
        Some(self.textures.get(&id)?.streamed.as_ref()?.ram_bytes())
    }

    /// The cache file a streamed texture reads its levels from, or `None`
    /// when it streams from memory or is not streamed.
    pub fn mip_cache_file(&self, id: u64) -> Option<&Path> {
        match &self.textures.get(&id)?.streamed.as_ref()?.source {
            MipSource::Disk { path, .. } => Some(path),
            MipSource::Memory(_) => None,
        }
    }

    /// Writes the chain to its cache file under the cache root (or finds
    /// it already there) and returns a source reading from it; `None` when
    /// there is no root or the file cannot be written, in which case the
    /// caller keeps the chain in RAM.
    fn cache_chain(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
        levels: &[(u32, u32, std::borrow::Cow<'_, [u8]>)],
        format: wgpu::TextureFormat,
    ) -> Option<MipSource> {
        let root = self.mip_cache_root.clone()?;
        let path = root.join(mip_cache_name(width, height, rgba, format));
        let dims: Vec<(u32, u32)> = levels.iter().map(|(w, h, _)| (*w, *h)).collect();
        if let Some(table) = read_mip_cache_table(&path, &dims, format) {
            // A hit is a use: without this the startup sweep
            // (`cache_sweep`) would judge the file by when it was written
            // and remove a chain that every session reads.
            crate::cache_sweep::touch(&path);
            return Some(MipSource::Disk { path, table });
        }
        let table = self.write_mip_cache(&root, &path, levels, format)?;
        Some(MipSource::Disk { path, table })
    }

    /// Writes `levels` as the cache file at `path`, returning its table;
    /// `None`, said once, when the directory cannot be written.
    fn write_mip_cache(
        &mut self,
        root: &Path,
        path: &Path,
        levels: &[(u32, u32, std::borrow::Cow<'_, [u8]>)],
        format: wgpu::TextureFormat,
    ) -> Option<Vec<(u64, u64)>> {
        let (bytes, table) = encode_mip_cache(levels, format);
        // Written beside its final name and renamed into place, so a
        // reader never sees a file that is half there.
        let tmp = path.with_extension(format!("mips.{}.tmp", std::process::id()));
        let written = std::fs::create_dir_all(root)
            .and_then(|()| std::fs::write(&tmp, &bytes))
            .and_then(|()| std::fs::rename(&tmp, path));
        match written {
            Ok(()) => Some(table),
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                if !self.mip_cache_warned {
                    self.mip_cache_warned = true;
                    tracing::warn!(
                        "[texture] cannot write the mip cache under {}: {e}; streamed textures \
                         keep their mip chains in RAM instead, and compressed ones are encoded \
                         at every load",
                        root.display()
                    );
                }
                None
            }
        }
    }

    /// The levels as they go to the GPU. For an uncompressed format, the
    /// chain as given. For a block format, the compressed levels: read back
    /// from the mip cache when the file is there -- so the encode is paid
    /// once per image and encoding, at first import, as the reference
    /// engines pay it at import -- and encoded, and written to the cache,
    /// when it is not.
    fn encoded_levels<'a>(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
        raw: Vec<(u32, u32, std::borrow::Cow<'a, [u8]>)>,
        format: wgpu::TextureFormat,
    ) -> Vec<(u32, u32, std::borrow::Cow<'a, [u8]>)> {
        if block_encoder(format).is_none() {
            return raw;
        }
        let dims: Vec<(u32, u32)> = raw.iter().map(|(w, h, _)| (*w, *h)).collect();
        let cached = self.mip_cache_root.as_ref().map(|root| {
            let path = root.join(mip_cache_name(width, height, rgba, format));
            (root.clone(), path)
        });
        if let Some((_, path)) = &cached {
            if let Some(table) = read_mip_cache_table(path, &dims, format) {
                if let Ok(levels) = read_all_levels(path, &dims, &table) {
                    crate::cache_sweep::touch(path);
                    return levels;
                }
            }
        }
        let shipped = self.shipped_mip_cache.as_ref().and_then(|lookup| {
            let bytes = lookup(&mip_cache_name(width, height, rgba, format))?;
            levels_from_bytes(&bytes, &dims, format)
        });
        let encoded: Vec<(u32, u32, std::borrow::Cow<'a, [u8]>)> = match shipped {
            Some(levels) => levels,
            None => {
                self.encodes += 1;
                raw.iter()
                    .map(|(w, h, pixels)| {
                        (
                            *w,
                            *h,
                            std::borrow::Cow::Owned(
                                encode_level(format, *w, *h, pixels).into_owned(),
                            ),
                        )
                    })
                    .collect()
            }
        };
        if let Some((root, path)) = cached {
            // Written now so the next load reads it; a streamed texture's
            // `cache_chain` then finds the very same file.
            let _ = self.write_mip_cache(&root, &path, &encoded, format);
        }
        encoded
    }

    fn create_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tex reg bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    /// Decodes an in-memory image file (PNG, JPEG, etc), uploads it as an
    /// RGBA8 texture with [`TextureImportSettings::raw`], and returns its
    /// assigned id.
    pub fn load_from_bytes(&mut self, bytes: &[u8]) -> Result<u64, String> {
        let img = image::load_from_memory(bytes).map_err(|e| format!("image decode: {e}"))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(self.load_from_rgba(width, height, &rgba))
    }

    /// Uploads already-decoded RGBA8 pixel data as a new texture with
    /// [`TextureImportSettings::raw`] -- linear, unmipped, clamped, exactly
    /// what every upload was before import settings existed -- and returns
    /// its assigned id. A texture that came from a file goes through
    /// [`load_with`](Self::load_with) with its sidecar's settings instead.
    pub fn load_from_rgba(&mut self, width: u32, height: u32, rgba: &[u8]) -> u64 {
        self.load_with(width, height, rgba, TextureImportSettings::raw())
    }

    /// Uploads already-decoded RGBA8 pixel data with the given import
    /// settings and returns the new texture's assigned id.
    pub fn load_with(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
        settings: TextureImportSettings,
    ) -> u64 {
        let tex = self.build(width, height, rgba, settings);
        let id = self.next_id;
        self.next_id += 1;
        self.textures.insert(id, tex);
        id
    }

    /// Rebuilds an already-loaded texture from new pixels, keeping its id and
    /// its import settings. Returns whether `id` was loaded.
    ///
    /// Hot reload uses this rather than `load_from_rgba` because `Material`
    /// stores the id: replacing under the same id updates every material using
    /// the texture at once. The new image may have different dimensions.
    ///
    /// The returned flag is `#[must_use]` for the same reason as
    /// `GpuMeshRegistry::replace`: `false` means an id a caller recorded at load
    /// time is no longer loaded, and dropping it turns that into a reload that
    /// appears to work and silently keeps the old pixels.
    #[must_use]
    pub fn replace(&mut self, id: u64, width: u32, height: u32, rgba: &[u8]) -> bool {
        let Some(settings) = self.textures.get(&id).map(|t| t.settings) else {
            return false;
        };
        self.replace_with(id, width, height, rgba, settings)
    }

    /// New pixels for a texture whose pixels change every frame -- a video --
    /// written into the texture it already has when the size is unchanged
    /// and it is one uncompressed level, so a frame costs one upload and
    /// nothing else: no new texture, no new bind group, no mip chain built on
    /// the CPU. Anything else falls back to [`replace`](Self::replace).
    /// `false` when `id` is not loaded.
    #[must_use]
    pub fn update_pixels(&mut self, id: u64, width: u32, height: u32, rgba: &[u8]) -> bool {
        let Some(tex) = self.textures.get_mut(&id) else {
            return false;
        };
        let in_place = tex.width == width
            && tex.height == height
            && tex.streamed.is_none()
            && !tex.settings.mipmaps
            && matches!(
                tex.format,
                wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb
            )
            && rgba.len() == (width * height * 4) as usize;
        if !in_place {
            return self.replace(id, width, height, rgba);
        }
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &tex._texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        true
    }

    /// [`replace`](Self::replace) with new import settings as well as new
    /// pixels -- what a reload driven by an edited sidecar needs.
    #[must_use]
    pub fn replace_with(
        &mut self,
        id: u64,
        width: u32,
        height: u32,
        rgba: &[u8],
        settings: TextureImportSettings,
    ) -> bool {
        if !self.textures.contains_key(&id) {
            return false;
        }
        let tex = self.build(width, height, rgba, settings);
        self.textures.insert(id, tex);
        true
    }

    fn build(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
        settings: TextureImportSettings,
    ) -> GpuTexture {
        let raw: Vec<(u32, u32, std::borrow::Cow<'_, [u8]>)> = if settings.mipmaps {
            mip_chain(width, height, rgba)
        } else {
            vec![(width, height, std::borrow::Cow::Borrowed(rgba))]
        };
        // The chain is built in RGBA8 and encoded afterwards, level by
        // level: a mip of a compressed image would otherwise be a blur of
        // block artefacts, and every reference engine filters first too.
        let format = self.upload_format(settings, width, height);
        let levels = self.encoded_levels(width, height, rgba, raw, format);
        // A streamed texture starts with only its small levels on the GPU
        // and keeps the chain -- in its cache file when there is one, else
        // in RAM -- to bring the rest in from. Streaming a texture with no
        // chain would have nothing to stream, so it uploads whole.
        let streamed = if settings.streaming && levels.len() > 1 {
            let dims: Vec<(u32, u32)> = levels.iter().map(|(w, h, _)| (*w, *h)).collect();
            let floor = residency_floor(&dims, format);
            let resident_base = (levels
                .iter()
                .position(|(w, h, _)| (*w).max(*h) <= STREAMING_INITIAL_MAX_DIM)
                .unwrap_or(levels.len() - 1) as u32)
                .min(floor);
            let source = self
                .cache_chain(width, height, rgba, &levels, format)
                .unwrap_or_else(|| {
                    MipSource::Memory(
                        levels
                            .iter()
                            .map(|(w, h, p)| (*w, *h, p.to_vec()))
                            .collect(),
                    )
                });
            Some(Streamed {
                source,
                dims,
                resident_base,
                floor,
                wanted_base: 0,
                pending: None,
            })
        } else {
            None
        };
        let resident_base = streamed.as_ref().map_or(0, |s| s.resident_base);
        let (texture, view) = self.upload_levels(&levels, resident_base, format);
        let (sampler, bind_group) = self.sampler_and_bind_group(&view, settings);
        GpuTexture {
            _texture: texture,
            _view: view,
            _sampler: sampler,
            bind_group,
            width,
            height,
            settings,
            format,
            streamed,
            generation: self.next_generation(),
        }
    }

    /// A fresh [`GpuTexture::generation`] for an object about to replace,
    /// or become, the one behind an id.
    fn next_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation += 1;
        generation
    }

    /// Creates the GPU texture holding `levels[resident_base..]` -- exactly
    /// those, so its footprint is what is resident -- writes them, and views
    /// the whole object. Level `resident_base` of the chain is level 0 of the
    /// object; the sampler never knows the larger levels exist.
    fn upload_levels(
        &self,
        levels: &[(u32, u32, std::borrow::Cow<'_, [u8]>)],
        resident_base: u32,
        format: wgpu::TextureFormat,
    ) -> (crate::profiler::TrackedTexture, wgpu::TextureView) {
        let resident = &levels[resident_base as usize..];
        let (width, height, _) = &resident[0];
        let texture = crate::profiler::create_tracked_texture(
            &self.device,
            &wgpu::TextureDescriptor {
                label: Some("user texture"),
                size: wgpu::Extent3d {
                    width: *width,
                    height: *height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: resident.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                // `COPY_SRC` because the skybox takes its image from here
                // (`WgpuSurface::set_skybox_from_texture`) rather than from
                // the pixels in `Assets`, which a material's upload may
                // already have released.
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
        );
        for (level, (w, h, pixels)) in resident.iter().enumerate() {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row_for(format, *w)),
                    rows_per_image: None,
                },
                copy_extent_for(format, *w, *h),
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// Whether `id` is a streamed texture.
    pub fn is_streaming(&self, id: u64) -> bool {
        self.textures.get(&id).is_some_and(|t| t.streamed.is_some())
    }

    /// A streamed texture's residency as `(resident_base, level_count)`:
    /// the chain index of the largest level on the GPU (`0` when fully
    /// resident) and how many levels the chain has. `None` for a texture
    /// that is not streamed.
    pub fn residency(&self, id: u64) -> Option<(u32, u32)> {
        let s = self.textures.get(&id)?.streamed.as_ref()?;
        Some((s.resident_base, s.dims.len() as u32))
    }

    /// Brings one more level of a streamed texture onto the GPU -- the next
    /// larger one. From a chain in RAM that happens now: the GPU object is
    /// rebuilt at the new size, the level written and the rest copied from
    /// the old object, so its footprint grows by exactly that level. From a
    /// cache file the level is read on a worker thread and the rebuild
    /// happens in [`Self::poll_reads`] once the bytes have arrived; until
    /// then residency is unchanged and the texture is left alone.
    ///
    /// Returns whether a level was brought in or its read started: `false`
    /// for a texture that is not streamed, is already fully resident, or has
    /// a read in flight.
    pub fn raise_residency(&mut self, id: u64) -> bool {
        let Some(tex) = self.textures.get(&id) else {
            return false;
        };
        let Some(streamed) = tex.streamed.as_ref() else {
            return false;
        };
        if streamed.pending.is_some() {
            return false;
        }
        let Some(base) = streamed.resident_base.checked_sub(1) else {
            return false;
        };
        match &streamed.source {
            MipSource::Memory(levels) => {
                let level = levels[base as usize].2.as_slice();
                let (texture, view) = self.rebuild_object(
                    &tex._texture,
                    streamed.resident_base,
                    base,
                    &streamed.dims,
                    Some(level),
                    tex.format,
                );
                self.install_object(id, base, texture, view);
                true
            }
            MipSource::Disk { path, table } => {
                let (offset, len) = table[base as usize];
                let pending = PendingRead::start(path.clone(), offset, len, base);
                let tex = self.textures.get_mut(&id).expect("looked up above");
                tex.streamed.as_mut().expect("checked above").pending = Some(pending);
                true
            }
        }
    }

    /// Drops the largest resident level of a streamed texture, rebuilding
    /// the GPU object without it: the levels that stay are copied
    /// GPU-to-GPU, so lowering reads nothing from RAM or disk. The smallest
    /// level always stays, so the texture keeps drawing something. Returns
    /// whether a level was dropped; `false` while a read is in flight, since
    /// that level was read for the residency it was requested at.
    pub fn lower_residency(&mut self, id: u64) -> bool {
        let Some(tex) = self.textures.get(&id) else {
            return false;
        };
        let Some(streamed) = tex.streamed.as_ref() else {
            return false;
        };
        if streamed.pending.is_some() {
            return false;
        }
        let base = streamed.resident_base + 1;
        // The floor is the last level for an uncompressed texture; for a
        // compressed one it stops short of the levels that cannot be an
        // object's level 0. See `Streamed::floor`.
        if base > streamed.floor {
            return false;
        }
        let (texture, view) = self.rebuild_object(
            &tex._texture,
            streamed.resident_base,
            base,
            &streamed.dims,
            None,
            tex.format,
        );
        self.install_object(id, base, texture, view);
        true
    }

    /// Uploads every level whose read has arrived, one texture at a time,
    /// and returns the ids whose residency changed. What the streaming
    /// system calls once a frame, before it decides the next step.
    ///
    /// A read that failed -- the cache file gone or short -- leaves
    /// residency where it is and is reported; the texture is then free to be
    /// asked again, which is what makes a file put back reachable.
    pub fn poll_reads(&mut self) -> Vec<u64> {
        let mut arrived: Vec<(u64, u32, std::io::Result<Vec<u8>>)> = Vec::new();
        for (id, tex) in &mut self.textures {
            let Some(streamed) = tex.streamed.as_mut() else {
                continue;
            };
            let Some(pending) = streamed.pending.as_ref() else {
                continue;
            };
            if let Some(result) = pending.take() {
                let base = pending.base;
                streamed.pending = None;
                arrived.push((*id, base, result));
            }
        }
        arrived.sort_unstable_by_key(|(id, _, _)| *id);

        let mut landed = Vec::new();
        for (id, base, result) in arrived {
            let bytes = match result {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::warn!(
                        "[texture] texture {id}'s mip cache file could not be read ({e}); its \
                         residency stays where it is"
                    );
                    continue;
                }
            };
            self.disk_reads += 1;
            self.disk_read_bytes += bytes.len() as u64;
            let tex = &self.textures[&id];
            let streamed = tex
                .streamed
                .as_ref()
                .expect("a read was pending on a streamed texture");
            let (texture, view) = self.rebuild_object(
                &tex._texture,
                streamed.resident_base,
                base,
                &streamed.dims,
                Some(&bytes),
                tex.format,
            );
            self.install_object(id, base, texture, view);
            landed.push(id);
        }
        landed
    }

    /// Whether a level is being read for `id` right now.
    pub fn has_pending_read(&self, id: u64) -> bool {
        self.textures
            .get(&id)
            .and_then(|t| t.streamed.as_ref())
            .is_some_and(|s| s.pending.is_some())
    }

    /// How many levels have been read out of cache files so far, and how
    /// many bytes: `(reads, bytes)`.
    pub fn disk_reads(&self) -> (u64, u64) {
        (self.disk_reads, self.disk_read_bytes)
    }

    /// Level 0 of the object behind `id`, read back from the GPU: what a
    /// test uses to see that the bytes a raise brought in -- or a lower
    /// kept -- are the ones on the GPU, rather than trusting the rebuild.
    #[cfg(test)]
    pub(crate) fn read_level0_for_testing(&self, id: u64) -> Vec<u8> {
        let tex = &self.textures[&id];
        crate::output::read_pixels(
            &self.device,
            &self.queue,
            &tex._texture,
            tex._texture.width(),
            tex._texture.height(),
        )
    }

    /// The GPU object for residency `base`, built out of the one for
    /// `old_base`: every level both hold is copied GPU-to-GPU, and when
    /// `base` is the lower of the two its one new level -- the largest -- is
    /// written from `new_level`. A raise therefore costs one level's write
    /// and a lower costs no upload at all; before this a change of residency
    /// re-uploaded every resident level from the chain, and from a cache
    /// file re-read them first.
    fn rebuild_object(
        &self,
        old: &wgpu::Texture,
        old_base: u32,
        base: u32,
        dims: &[(u32, u32)],
        new_level: Option<&[u8]>,
        format: wgpu::TextureFormat,
    ) -> (crate::profiler::TrackedTexture, wgpu::TextureView) {
        let count = dims.len() as u32 - base;
        let (width, height) = dims[base as usize];
        let texture = crate::profiler::create_tracked_texture(
            &self.device,
            &wgpu::TextureDescriptor {
                label: Some("user texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("streamed texture rebuild"),
            });
        for level in 0..count {
            let chain = base + level;
            let (w, h) = dims[chain as usize];
            let extent = copy_extent_for(format, w, h);
            let destination = wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            };
            if chain >= old_base {
                encoder.copy_texture_to_texture(
                    wgpu::ImageCopyTexture {
                        texture: old,
                        mip_level: chain - old_base,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    destination,
                    extent,
                );
            } else {
                let bytes =
                    new_level.expect("a level the old object lacks is the one being brought in");
                self.queue.write_texture(
                    destination,
                    bytes,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row_for(format, w)),
                        rows_per_image: None,
                    },
                    extent,
                );
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// Makes `texture` the object behind `id`, at residency `base`.
    fn install_object(
        &mut self,
        id: u64,
        base: u32,
        texture: crate::profiler::TrackedTexture,
        view: wgpu::TextureView,
    ) {
        let settings = self.textures[&id].settings;
        let (sampler, bind_group) = self.sampler_and_bind_group(&view, settings);
        let generation = self.next_generation();
        let tex = self.textures.get_mut(&id).expect("looked up above");
        tex.streamed.as_mut().expect("checked above").resident_base = base;
        // The bind group is what a draw binds; swapping the texture and view
        // without it would leave every material sampling the old object,
        // which the old view keeps alive -- the footprint would grow and
        // nothing on screen would change.
        tex._texture = texture;
        tex._view = view;
        tex._sampler = sampler;
        tex.bind_group = bind_group;
        tex.generation = generation;
    }

    /// Brings one level in on the streamed texture that has been waiting
    /// longest -- round-robin over every streamed texture that is not yet
    /// fully resident -- and returns its id. `None` when every streamed
    /// texture is fully resident. What the progressive loader did before
    /// wants existed; [`step_streaming`](Self::step_streaming) with every
    /// want at `0` and no budget does the same.
    pub fn raise_next_pending(&mut self) -> Option<u64> {
        let mut pending: Vec<u64> = self
            .textures
            .iter()
            .filter(|(_, t)| {
                t.streamed
                    .as_ref()
                    .is_some_and(|s| s.resident_base > 0 && s.pending.is_none())
            })
            .map(|(id, _)| *id)
            .collect();
        pending.sort_unstable();
        let next = *pending
            .iter()
            .find(|id| **id > self.last_raised)
            .or_else(|| pending.first())?;
        self.last_raised = next;
        self.raise_residency(next).then_some(next)
    }

    /// Records, for every streamed texture, how large it is drawn on screen
    /// this frame -- `pixels` is the largest on-screen extent of anything
    /// using it -- and so which level it wants resident (see
    /// [`wanted_base_for`]). A streamed texture absent from `wants` is
    /// wanted whole: nothing measured says it can be smaller.
    ///
    /// `bias` is the project's mip bias, in levels; positive wants smaller.
    pub fn set_wants(&mut self, wants: &HashMap<u64, f32>, bias: i32) {
        for (id, tex) in &mut self.textures {
            let Some(streamed) = tex.streamed.as_mut() else {
                continue;
            };
            streamed.wanted_base = match wants.get(id) {
                Some(pixels) => wanted_base_for(&streamed.dims, *pixels, bias).min(streamed.floor),
                None => 0,
            };
        }
    }

    /// The chain index a streamed texture currently wants resident; `None`
    /// for a texture that is not streamed.
    pub fn wanted(&self, id: u64) -> Option<u32> {
        Some(self.textures.get(&id)?.streamed.as_ref()?.wanted_base)
    }

    /// Bytes the streamed textures hold on the GPU between them, as the
    /// profiler counts each one.
    pub fn streamed_resident_bytes(&self) -> u64 {
        self.textures
            .values()
            .filter(|t| t.streamed.is_some())
            .map(|t| t._texture.size_bytes())
            .sum()
    }

    /// How many streamed textures hold less than they want.
    pub fn textures_below_wanted(&self) -> u32 {
        self.textures
            .values()
            .filter(|t| {
                t.streamed
                    .as_ref()
                    .is_some_and(|s| s.resident_base > s.wanted_base)
            })
            .count() as u32
    }

    /// Moves residency one level towards what the wants and the budget
    /// say, on one texture, and reports what it did. What the streaming
    /// system calls once per frame; one level per frame for the reason the
    /// progressive loader gave -- each change is an allocation and a
    /// re-upload, and spreading them is what keeps a frame from stalling.
    ///
    /// In order of precedence, as Unreal's pool and Unity's budget behave:
    ///
    /// 1. **Over budget**: drop the largest resident level of the texture
    ///    holding the most it does not want (the one furthest from the
    ///    camera), or, when every texture is at or below its want, of the
    ///    one with the largest resident level. The budget wins over wants.
    /// 2. **A texture wants more**: bring the next level in on the one
    ///    furthest below its want, ties taken in turn -- if that level fits
    ///    the budget. If it does not, a texture holding a level it does
    ///    not want gives that up instead, so room appears next frame.
    /// 3. **A texture holds more than a level above its want**: drop its
    ///    largest level. One level of slack, so a texture whose want
    ///    flickers across a level boundary as the camera moves is not
    ///    rebuilt every frame.
    ///
    /// `budget` is the most bytes streamed textures may hold between them;
    /// `None` is no limit. Returns `None` when nothing needed doing.
    pub fn step_streaming(&mut self, budget: Option<u64>) -> Option<StreamingStep> {
        // (id, resident_base, wanted_base, last chain index, largest resident level bytes, next level bytes)
        let mut streamed: Vec<(u64, u32, u32, u32, u64, u64)> = self
            .textures
            .iter()
            .filter_map(|(id, t)| {
                let s = t.streamed.as_ref()?;
                // A texture with a level in flight is neither raised again
                // nor lowered: the level was read for the residency it was
                // asked at, and lands there in `poll_reads`.
                if s.pending.is_some() {
                    return None;
                }
                // `floor`, not the last level: a compressed texture's
                // residency cannot go below it, so for the budget it is the
                // "cannot be lowered" end, or the step would pick it as a
                // victim every frame and lower nothing.
                let last = s.floor;
                let format = t.format;
                let level_bytes = |i: u32| {
                    let (w, h) = s.dims[i as usize];
                    crate::profiler::level_bytes(format, w, h)
                };
                let next = s.resident_base.checked_sub(1).map_or(0, level_bytes);
                Some((
                    *id,
                    s.resident_base,
                    s.wanted_base,
                    last,
                    level_bytes(s.resident_base),
                    next,
                ))
            })
            .collect();
        streamed.sort_unstable_by_key(|s| s.0);
        let resident = self.streamed_resident_bytes();

        // The texture holding the most it does not want, largest level
        // first among equals; `None` when none can be lowered.
        let most_surplus = |min_surplus: u32| {
            streamed
                .iter()
                .filter(|(_, base, wanted, last, _, _)| {
                    base < last && wanted.saturating_sub(*base) >= min_surplus
                })
                .max_by_key(|(id, base, wanted, _, bytes, _)| {
                    (wanted.saturating_sub(*base), *bytes, std::cmp::Reverse(*id))
                })
                .map(|s| s.0)
        };

        if let Some(budget) = budget {
            if resident > budget {
                let victim = most_surplus(1).or_else(|| {
                    streamed
                        .iter()
                        .filter(|(_, base, _, last, _, _)| base < last)
                        .max_by_key(|(id, _, _, _, bytes, _)| (*bytes, std::cmp::Reverse(*id)))
                        .map(|s| s.0)
                })?;
                return self
                    .lower_residency(victim)
                    .then_some(StreamingStep::Lowered(victim));
            }
        }

        // The texture furthest below its want; ties go to the one after the
        // last raised, so textures brought up together each get a turn.
        let deficit = streamed
            .iter()
            .filter(|(_, base, wanted, _, _, _)| base > wanted)
            .map(|(id, base, wanted, _, _, _)| (base - wanted, *id))
            .max_by_key(|(gap, _)| *gap)
            .map(|(gap, _)| gap);
        if let Some(gap) = deficit {
            let mut candidates: Vec<&(u64, u32, u32, u32, u64, u64)> = streamed
                .iter()
                .filter(|(_, base, wanted, _, _, _)| base > wanted && base - wanted == gap)
                .collect();
            candidates.sort_unstable_by_key(|s| s.0);
            let next = candidates
                .iter()
                .find(|s| s.0 > self.last_raised)
                .or_else(|| candidates.first())
                .copied()
                .expect("a deficit means at least one candidate");
            let fits = budget.is_none_or(|b| resident + next.5 <= b);
            if fits {
                self.last_raised = next.0;
                return self
                    .raise_residency(next.0)
                    .then_some(StreamingStep::Raised(next.0));
            }
            if let Some(victim) = most_surplus(1) {
                return self
                    .lower_residency(victim)
                    .then_some(StreamingStep::Lowered(victim));
            }
            return None;
        }

        let victim = most_surplus(2)?;
        self.lower_residency(victim)
            .then_some(StreamingStep::Lowered(victim))
    }

    fn sampler_and_bind_group(
        &self,
        view: &wgpu::TextureView,
        settings: TextureImportSettings,
    ) -> (wgpu::Sampler, wgpu::BindGroup) {
        let (mag_filter, min_filter) = match settings.filter {
            TextureFilter::Linear => (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear),
            TextureFilter::Nearest => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest),
        };
        let address_mode = match settings.wrap {
            TextureWrap::Repeat => wgpu::AddressMode::Repeat,
            TextureWrap::Clamp => wgpu::AddressMode::ClampToEdge,
            TextureWrap::Mirror => wgpu::AddressMode::MirrorRepeat,
        };
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("user texture sampler"),
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter,
            min_filter,
            // Between mip levels, not within one: with a single level this
            // never engages, and with a chain it is what stops the level
            // switch from showing as a seam.
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("user tex bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        (sampler, bind_group)
    }

    /// Looks up a previously loaded texture's bind group by id.
    pub fn get_bind_group(&self, id: u64) -> Option<&wgpu::BindGroup> {
        self.textures.get(&id).map(|t| &t.bind_group)
    }

    /// Looks up a previously loaded texture's raw view by id. Used by
    /// terrain rendering, which needs several textures' views in one
    /// bind group rather than this registry's own 2-entry (texture,
    /// sampler) layout.
    pub fn get_view(&self, id: u64) -> Option<&wgpu::TextureView> {
        self.textures.get(&id).map(|t| &t._view)
    }

    /// Looks up a previously loaded texture's pixel dimensions by id.
    pub fn get_size(&self, id: u64) -> Option<(u32, u32)> {
        self.textures.get(&id).map(|t| (t.width, t.height))
    }

    /// The GPU object currently behind `id`: what a consumer that keeps its
    /// own copy (the skybox) copies from. Its level 0 is the largest
    /// *resident* level of a streamed texture, and its size says which.
    pub fn get_texture(&self, id: u64) -> Option<&wgpu::Texture> {
        self.textures.get(&id).map(|t| &*t._texture)
    }

    /// A number that changes whenever the object behind `id` is rebuilt
    /// (hot reload, a streamed level in or out) and never otherwise, so a
    /// consumer holding a copy of it knows when to copy again. See
    /// [`GpuTexture::generation`].
    pub fn generation(&self, id: u64) -> Option<u64> {
        self.textures.get(&id).map(|t| t.generation)
    }

    /// The import settings a texture was uploaded with.
    pub fn get_settings(&self, id: u64) -> Option<TextureImportSettings> {
        self.textures.get(&id).map(|t| t.settings)
    }

    /// What the settings turned into on the GPU: the texture's format and
    /// how many mip levels it holds. What a test can observe; a sampler's
    /// filter and wrap can only be seen by rendering with it.
    pub fn get_gpu_shape(&self, id: u64) -> Option<(wgpu::TextureFormat, u32)> {
        self.textures
            .get(&id)
            .map(|t| (t._texture.format(), t._texture.mip_level_count()))
    }

    /// The GPU object's own level-0 dimensions and footprint in bytes, as the
    /// profiler counts it. For an unstreamed texture that is the image's
    /// size; for a streamed one it is the largest *resident* level, which is
    /// what makes residency observable from outside without trusting the
    /// registry's own bookkeeping.
    pub fn get_gpu_footprint(&self, id: u64) -> Option<(u32, u32, u64)> {
        self.textures.get(&id).map(|t| {
            (
                t._texture.width(),
                t._texture.height(),
                t._texture.size_bytes(),
            )
        })
    }
}

/// The full mip chain of an RGBA8 image, level 0 first, down to 1x1.
///
/// Built on the CPU with a box filter rather than on the GPU with a render
/// pass per level: user textures are uploaded once, the chain costs a third
/// of the image again in bytes, and a compute or blit path would need a
/// pipeline and a bind group per level for the same result.
fn mip_chain(width: u32, height: u32, rgba: &[u8]) -> Vec<(u32, u32, std::borrow::Cow<'_, [u8]>)> {
    let mut levels: Vec<(u32, u32, std::borrow::Cow<'_, [u8]>)> =
        vec![(width, height, std::borrow::Cow::Borrowed(rgba))];
    let (mut w, mut h) = (width, height);
    while w > 1 || h > 1 {
        let (nw, nh, next) = {
            let (_, _, prev) = levels.last().expect("level 0 is always present");
            downsample(w, h, prev)
        };
        levels.push((nw, nh, std::borrow::Cow::Owned(next)));
        w = nw;
        h = nh;
    }
    levels
}

/// Halves an RGBA8 image with a 2x2 box filter, rounding odd dimensions up
/// so a 3-wide row still yields 2 texels, the second averaging the clamped
/// edge with itself.
fn downsample(width: u32, height: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let nw = (width / 2).max(1);
    let nh = (height / 2).max(1);
    let mut out = Vec::with_capacity((nw * nh * 4) as usize);
    let texel = |x: u32, y: u32| -> [u32; 4] {
        let x = x.min(width - 1);
        let y = y.min(height - 1);
        let i = ((y * width + x) * 4) as usize;
        [
            rgba[i] as u32,
            rgba[i + 1] as u32,
            rgba[i + 2] as u32,
            rgba[i + 3] as u32,
        ]
    };
    for y in 0..nh {
        for x in 0..nw {
            let (sx, sy) = (x * 2, y * 2);
            let a = texel(sx, sy);
            let b = texel(sx + 1, sy);
            let c = texel(sx, sy + 1);
            let d = texel(sx + 1, sy + 1);
            for ch in 0..4 {
                out.push(((a[ch] + b[ch] + c[ch] + d[ch] + 2) / 4) as u8);
            }
        }
    }
    (nw, nh, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::WgpuSurface;

    fn make_registry() -> GpuTextureRegistry {
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        GpuTextureRegistry::new(device, queue)
    }

    /// A registry whose streamed textures cache their chains under a fresh
    /// scratch directory named for the test.
    fn make_registry_with_cache(test_name: &str) -> (GpuTextureRegistry, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("bse_mip_cache_{test_name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut reg = make_registry();
        reg.set_mip_cache_root(Some(dir.clone()));
        (reg, dir)
    }

    fn mips_files(dir: &Path) -> Vec<PathBuf> {
        let Ok(read) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = read
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "mips"))
            .collect();
        files.sort();
        files
    }

    /// The generation is what a consumer holding its own copy of an object
    /// (the skybox) watches, so it must change on exactly the events that
    /// rebuild the object -- a level in, a level out, a reload -- and on
    /// nothing else: another texture's rebuild must not touch it, or the
    /// skybox would copy itself again every time anything streamed.
    #[test]
    fn the_generation_changes_on_every_rebuild_of_an_object_and_only_its_own() {
        let mut reg = make_registry();
        let pixels = vec![9u8; 256 * 256 * 4];
        let streamed_id = reg.load_with(256, 256, &pixels, streamed());
        let plain_id = reg.load_from_rgba(2, 2, &[1u8; 16]);
        let (g0, p0) = (
            reg.generation(streamed_id).unwrap(),
            reg.generation(plain_id).unwrap(),
        );
        assert_ne!(g0, p0, "every object gets its own number");
        assert_eq!(reg.generation(999), None, "an id nothing loaded has none");

        assert!(reg.raise_residency(streamed_id));
        let g1 = reg.generation(streamed_id).unwrap();
        assert_ne!(g1, g0, "a level in rebuilds the object");
        assert!(reg.lower_residency(streamed_id));
        let g2 = reg.generation(streamed_id).unwrap();
        assert!(g2 != g1 && g2 != g0, "a level out rebuilds it again");
        assert!(reg.replace(streamed_id, 256, 256, &pixels));
        let g3 = reg.generation(streamed_id).unwrap();
        assert!(g3 != g2, "a reload rebuilds it");
        assert_eq!(
            reg.generation(plain_id),
            Some(p0),
            "none of which touches another texture's object"
        );
        // A reload rebuilds a streamed texture with only its initial levels
        // resident, so the object handed out is the current one *at its
        // resident size* -- 64 across, not the image's 256 -- which is what
        // the skybox copies and shows until the levels stream back in.
        assert!(
            reg.get_texture(streamed_id)
                .is_some_and(|t| t.width() == STREAMING_INITIAL_MAX_DIM),
            "the object handed out is the current one, at its resident size"
        );
    }

    /// With a cache directory, a streamed texture's chain goes to one
    /// `.mips` file and **nothing of it stays in RAM** -- the saving the
    /// cache exists for, measured rather than assumed -- while the GPU
    /// side is exactly what it is without a cache. Identical pixels share
    /// the file; different pixels get their own; an unstreamed texture
    /// writes none.
    #[test]
    fn a_cache_dir_moves_the_chain_out_of_ram_into_one_file_per_image() {
        let (mut reg, dir) = make_registry_with_cache("one_file");
        let pixels = vec![200u8; 256 * 256 * 4];

        // Premise: without a cache the chain is held in RAM, and it is the
        // whole chain.
        let ram_reg = {
            let mut r = make_registry();
            let id = r.load_with(256, 256, &pixels, streamed());
            (r, id)
        };
        assert!(
            within_one_percent(
                ram_reg.0.chain_ram_bytes(ram_reg.1).unwrap(),
                chain_bytes(256, 9)
            ),
            "premise: a registry without a cache holds the whole chain in RAM"
        );
        assert_eq!(ram_reg.0.mip_cache_file(ram_reg.1), None);

        let id = reg.load_with(256, 256, &pixels, streamed());
        assert_eq!(
            reg.chain_ram_bytes(id),
            Some(0),
            "the chain is on disk, not in RAM"
        );
        assert_eq!(
            reg.residency(id),
            Some((2, 9)),
            "and residency is as without a cache"
        );
        assert_eq!(
            reg.get_gpu_footprint(id).map(|f| (f.0, f.1)),
            Some((64, 64))
        );
        let files = mips_files(&dir);
        assert_eq!(files.len(), 1, "one cache file: {files:?}");
        assert_eq!(reg.mip_cache_file(id), Some(files[0].as_path()));
        let expected_len = (chain_bytes(256, 9) as usize) + 8 + 4 + 9 * 24;
        assert!(
            within_one_percent(
                std::fs::metadata(&files[0]).unwrap().len(),
                expected_len as u64
            ),
            "the file holds the header and every level"
        );

        // The same pixels again: the file is reused, not written twice --
        // a byte changed in it survives, which a rewrite would undo -- and
        // the reuse is recorded as a use, so a file read every session is
        // not what the startup sweep (`cache_sweep`) judges by its age.
        // The file is aged past the sweep's limit *after* the tampering
        // write (which sets its own timestamp: aged before it, the
        // assertion below passed with the touch deleted), so "refreshed"
        // is the reuse's doing and nothing else's.
        let len = std::fs::metadata(&files[0]).unwrap().len();
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(&files[0])
                .unwrap();
            f.seek(SeekFrom::Start(len - 1)).unwrap();
            f.write_all(&[123]).unwrap();
        }
        let aged = std::time::SystemTime::now() - 30 * crate::cache_sweep::DAY;
        crate::cache_sweep::set_mtime(&files[0], aged);
        let again = reg.load_with(256, 256, &pixels, streamed());
        assert_eq!(mips_files(&dir).len(), 1);
        assert_eq!(reg.mip_cache_file(again), Some(files[0].as_path()));
        assert_eq!(
            std::fs::read(&files[0]).unwrap().last(),
            Some(&123),
            "the file was reused, not rewritten"
        );
        assert!(
            crate::cache_sweep::mtime(&files[0])
                > std::time::SystemTime::now() - crate::cache_sweep::DAY,
            "and the reuse refreshed its modification time (was {aged:?})"
        );

        // Different pixels: a second file. An unstreamed texture: none.
        let other = reg.load_with(256, 256, &vec![9u8; 256 * 256 * 4], streamed());
        assert_eq!(mips_files(&dir).len(), 2);
        assert_ne!(reg.mip_cache_file(other), reg.mip_cache_file(id));
        let plain = reg.load_with(256, 256, &pixels, TextureImportSettings::default());
        assert_eq!(mips_files(&dir).len(), 2);
        assert_eq!(reg.chain_ram_bytes(plain), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Runs `poll_reads` until the read in flight on `id` has landed or
    /// failed: a worker thread reads the level, and a test has no way to
    /// know when it is done but to ask. Returns what landed.
    fn wait_for_read(reg: &mut GpuTextureRegistry, id: u64) -> Vec<u64> {
        let start = std::time::Instant::now();
        while reg.has_pending_read(id) {
            let landed = reg.poll_reads();
            if !landed.is_empty() {
                return landed;
            }
            assert!(
                start.elapsed() < std::time::Duration::from_secs(10),
                "the read never finished"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        Vec::new()
    }

    /// A raise from a cache file reads exactly the level it brings in, on a
    /// worker thread -- residency is unchanged, and the texture is left
    /// alone, until the read lands in `poll_reads` -- and what lands is what
    /// the file holds: bytes changed in the file are what the GPU gets.
    /// Lowering reads nothing: the levels that stay are copied on the GPU.
    /// A file that has gone leaves residency where it is and the texture
    /// free to be asked again.
    #[test]
    fn a_raise_reads_one_level_off_the_frame_thread_and_a_lower_reads_nothing() {
        let (mut reg, dir) = make_registry_with_cache("read_back");
        let id = reg.load_with(256, 256, &vec![50u8; 256 * 256 * 4], streamed());
        let file = reg.mip_cache_file(id).unwrap().to_path_buf();
        assert_eq!(reg.residency(id), Some((2, 9)));
        assert_eq!(
            reg.disk_reads(),
            (0, 0),
            "premise: the upload read nothing back"
        );

        // The level a raise will bring in next is the 128 one (index 1):
        // overwrite it in the file and see the read return the new bytes.
        let dims: Vec<(u32, u32)> = (0..9).map(|i| (256 >> i, 256 >> i)).collect();
        let table = read_mip_cache_table(&file, &dims, wgpu::TextureFormat::Rgba8UnormSrgb)
            .expect("the file must parse");
        let (offset, len) = table[1];
        assert_eq!(len, 128 * 128 * 4);
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut f = std::fs::OpenOptions::new().write(true).open(&file).unwrap();
            f.seek(SeekFrom::Start(offset)).unwrap();
            f.write_all(&vec![7u8; len as usize]).unwrap();
        }
        let streamed = reg.textures[&id].streamed.as_ref().unwrap();
        let levels = streamed.levels_from(1).expect("readable");
        assert_eq!(levels.len(), 8, "levels 128 down to 1");
        assert!(
            levels[0].2.iter().all(|b| *b == 7) && levels[1].2.iter().all(|b| *b == 50),
            "premise: the file holds the tampered 128 level over the untouched 64"
        );

        let before = reg.generation(id).unwrap();
        assert!(reg.raise_residency(id), "the read starts");
        assert_eq!(
            reg.residency(id),
            Some((2, 9)),
            "and nothing changes until it lands"
        );
        assert!(reg.has_pending_read(id));
        assert!(!reg.raise_residency(id), "one read in flight per texture");
        assert!(!reg.lower_residency(id), "and no lowering under it");
        assert_eq!(
            reg.step_streaming(None),
            None,
            "step_streaming leaves a texture with a read in flight alone"
        );

        assert_eq!(wait_for_read(&mut reg, id), vec![id], "the read lands");
        assert_eq!(reg.residency(id), Some((1, 9)));
        assert_eq!(reg.get_gpu_footprint(id).map(|f| f.0), Some(128));
        assert_ne!(
            reg.generation(id),
            Some(before),
            "a landed level is a rebuild of the object"
        );
        assert_eq!(
            reg.disk_reads(),
            (1, 128 * 128 * 4),
            "exactly the one level was read, and no more"
        );
        assert!(
            reg.read_level0_for_testing(id).iter().all(|b| *b == 7),
            "the GPU holds the file's bytes, tampering and all"
        );

        assert!(reg.lower_residency(id));
        assert_eq!(reg.residency(id), Some((2, 9)));
        assert_eq!(
            reg.disk_reads(),
            (1, 128 * 128 * 4),
            "lowering read nothing from the file"
        );
        assert_eq!(reg.chain_ram_bytes(id), Some(0), "still nothing in RAM");
        assert!(
            reg.read_level0_for_testing(id).iter().all(|b| *b == 50),
            "the 64 level that stays was copied on the GPU from the object that had it"
        );

        // The file gone: the read fails, residency stays, and the texture is
        // free to be asked again.
        std::fs::remove_file(&file).unwrap();
        assert!(reg.raise_residency(id), "the read starts");
        assert!(
            wait_for_read(&mut reg, id).is_empty(),
            "no file, no level to bring in"
        );
        assert_eq!(reg.residency(id), Some((2, 9)));
        assert!(!reg.has_pending_read(id), "and it can be asked again");
        assert_eq!(reg.disk_reads(), (1, 128 * 128 * 4));
        assert_eq!(reg.get_gpu_footprint(id).map(|f| f.0), Some(64));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A texture asking for BC1 goes up in BC1 blocks, at half a byte a
    /// texel where RGBA8 is four, with every level a whole number of
    /// blocks -- the 1x1 level is one 8-byte block, not half a byte. BC3 is
    /// a byte a texel. And on a device without block compression the same
    /// request goes up as RGBA8, encoding nothing.
    #[test]
    fn a_compressed_texture_uploads_in_blocks_and_falls_back_without_support() {
        use bsengine_core::TextureCompression;
        let mut reg = make_registry();
        assert!(
            reg.bc_supported(),
            "premise: this device must have block compression"
        );
        let pixels = vec![200u8; 64 * 64 * 4];
        let bc1 = TextureImportSettings {
            compression: TextureCompression::Bc1,
            ..Default::default()
        };
        let id = reg.load_with(64, 64, &pixels, bc1);
        assert_eq!(
            reg.get_gpu_shape(id),
            Some((wgpu::TextureFormat::Bc1RgbaUnormSrgb, 7)),
            "the setting reaches the format, with the whole chain"
        );
        let (w, h, bytes) = reg.get_gpu_footprint(id).unwrap();
        assert_eq!((w, h), (64, 64));
        // Levels 64, 32, 16, 8, 4, 2, 1: 256 + 64 + 16 + 4 + 1 + 1 + 1 blocks.
        assert_eq!(bytes, 343 * 8, "whole blocks per level, 8 bytes each");
        let plain = reg.load_from_rgba(64, 64, &pixels);
        assert_eq!(reg.get_gpu_footprint(plain).unwrap().2, 64 * 64 * 4);
        assert_eq!(reg.encodes(), 1, "one chain was encoded");

        let bc3 = TextureImportSettings {
            compression: TextureCompression::Bc3,
            mipmaps: false,
            ..Default::default()
        };
        let id3 = reg.load_with(64, 64, &pixels, bc3);
        assert_eq!(
            reg.get_gpu_shape(id3),
            Some((wgpu::TextureFormat::Bc3RgbaUnormSrgb, 1))
        );
        assert_eq!(reg.get_gpu_footprint(id3).unwrap().2, 256 * 16);
        assert_eq!(reg.encodes(), 2);

        reg.pretend_bc_unsupported_for_testing();
        let fallback = reg.load_with(64, 64, &pixels, bc1);
        assert_eq!(
            reg.get_gpu_shape(fallback),
            Some((wgpu::TextureFormat::Rgba8UnormSrgb, 7)),
            "without the feature the texture goes up uncompressed"
        );
        // 4 bytes times 4096 + 1024 + 256 + 64 + 16 + 4 + 1 texels.
        assert_eq!(reg.get_gpu_footprint(fallback).unwrap().2, 4 * 5461);
        assert_eq!(reg.encodes(), 2, "and nothing was encoded for it");
    }

    /// A block-compressed object's level 0 must be whole blocks -- wgpu
    /// refuses to create a 2x2 BC1 texture -- so a streamed compressed
    /// chain's residency stops at the last level of the run from level 0
    /// that is a multiple of 4 on each side, where an uncompressed one goes
    /// down to 1x1; the wants and the budget stop there with it. And an
    /// image that is not a multiple of 4 to begin with is not compressed at
    /// all: it goes up as RGBA8 with a warning, as Unity does.
    #[test]
    fn a_compressed_texture_keeps_whole_blocks_at_level_0() {
        use bsengine_core::TextureCompression;
        let mut reg = make_registry();
        assert!(
            reg.bc_supported(),
            "premise: block compression on this device"
        );
        let pixels = vec![90u8; 96 * 96 * 4];
        let streamed = |compression| TextureImportSettings {
            compression,
            streaming: true,
            ..Default::default()
        };
        let bc1 = reg.load_with(
            64,
            64,
            &pixels[..64 * 64 * 4],
            streamed(TextureCompression::Bc1),
        );
        let plain = reg.load_with(
            64,
            64,
            &pixels[..64 * 64 * 4],
            streamed(TextureCompression::None),
        );
        // 96, 48, 24, 12 are whole blocks; 6, 3, 2, 1 are not.
        let odd = reg.load_with(96, 96, &pixels, streamed(TextureCompression::Bc1));
        let lowest = |reg: &mut GpuTextureRegistry, id: u64| {
            while reg.lower_residency(id) {}
            reg.residency(id).unwrap().0
        };
        assert_eq!(
            lowest(&mut reg, plain),
            6,
            "premise: an uncompressed chain lowers all the way to 1x1"
        );
        assert_eq!(lowest(&mut reg, bc1), 4, "a compressed one stops at 4x4");
        assert_eq!(
            reg.get_gpu_shape(bc1),
            Some((wgpu::TextureFormat::Bc1RgbaUnormSrgb, 3)),
            "holding 4, 2 and 1 as the mips of the 4x4 object"
        );
        assert_eq!(
            lowest(&mut reg, odd),
            3,
            "96 stops at 12x12, the last whole-block level"
        );

        // The wants clamp to the same floor, so the step never asks for
        // less than can be built...
        let wants = HashMap::from([(bc1, 1.0), (plain, 1.0), (odd, 1.0)]);
        reg.set_wants(&wants, 0);
        assert_eq!(
            reg.wanted(plain),
            Some(6),
            "premise: a 1-pixel want reaches the last level"
        );
        assert_eq!(reg.wanted(bc1), Some(4));
        assert_eq!(reg.wanted(odd), Some(3));
        // ... and a budget nothing fits in finds nothing left to drop,
        // rather than a 2x2 BC1 object to fail on.
        assert_eq!(reg.step_streaming(Some(0)), None);

        let before = reg.encodes();
        let unaligned = reg.load_with(
            66,
            40,
            &pixels[..66 * 40 * 4],
            TextureImportSettings {
                compression: TextureCompression::Bc1,
                ..Default::default()
            },
        );
        assert_eq!(
            reg.get_gpu_shape(unaligned).map(|s| s.0),
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            "66 is not a multiple of 4: uncompressed"
        );
        assert_eq!(reg.encodes(), before, "and nothing was encoded for it");
    }

    /// The encode is paid once. With a cache directory the compressed chain
    /// is written to a cache file at the first upload and read back at the
    /// next, encoding nothing; the file holds the blocks, not the pixels,
    /// and is not mistaken for the RGBA8 chain of the same image, which is
    /// its own file. A streamed compressed texture raises its levels out
    /// of that same file, compressed.
    #[test]
    fn a_compressed_chain_is_encoded_once_and_read_from_the_cache_after() {
        use bsengine_core::TextureCompression;
        let (mut reg, dir) = make_registry_with_cache("compressed");
        assert!(
            reg.bc_supported(),
            "premise: block compression on this device"
        );
        let pixels = vec![77u8; 128 * 128 * 4];
        let bc1 = TextureImportSettings {
            compression: TextureCompression::Bc1,
            ..Default::default()
        };
        let first = reg.load_with(128, 128, &pixels, bc1);
        assert_eq!(reg.encodes(), 1);
        let files = mips_files(&dir);
        assert_eq!(files.len(), 1, "one cache file: {files:?}");
        let dims: Vec<(u32, u32)> = (0..8).map(|i| (128 >> i, 128 >> i)).collect();
        let table = read_mip_cache_table(&files[0], &dims, wgpu::TextureFormat::Bc1RgbaUnormSrgb)
            .expect("the file holds the BC1 chain");
        assert_eq!(
            table[0].1,
            32 * 32 * 8,
            "level 0 is 32x32 blocks of 8 bytes"
        );
        assert!(
            read_mip_cache_table(&files[0], &dims, wgpu::TextureFormat::Rgba8UnormSrgb).is_none(),
            "and is not read as an RGBA8 chain"
        );

        let second = reg.load_with(128, 128, &pixels, bc1);
        assert_eq!(reg.encodes(), 1, "the second upload came out of the cache");
        assert_eq!(mips_files(&dir).len(), 1);
        assert_eq!(reg.get_gpu_footprint(second), reg.get_gpu_footprint(first));

        reg.load_with(
            128,
            128,
            &pixels,
            TextureImportSettings {
                streaming: true,
                ..Default::default()
            },
        );
        assert_eq!(
            mips_files(&dir).len(),
            2,
            "the RGBA8 chain of the same image is its own file"
        );

        let streamed = reg.load_with(
            128,
            128,
            &pixels,
            TextureImportSettings {
                compression: TextureCompression::Bc1,
                streaming: true,
                ..Default::default()
            },
        );
        assert_eq!(reg.encodes(), 1, "still the cached blocks");
        assert_eq!(mips_files(&dir).len(), 2, "and the same file");
        assert_eq!(
            reg.residency(streamed),
            Some((1, 8)),
            "the 64 level is resident"
        );
        assert!(reg.raise_residency(streamed));
        assert_eq!(wait_for_read(&mut reg, streamed), vec![streamed]);
        assert_eq!(reg.residency(streamed), Some((0, 8)));
        assert_eq!(
            reg.disk_reads(),
            (1, 32 * 32 * 8),
            "the level read in is the compressed one"
        );
        assert_eq!(
            reg.get_gpu_shape(streamed).map(|s| s.0),
            Some(wgpu::TextureFormat::Bc1RgbaUnormSrgb)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Not a check but a measurement, so `#[ignore]`d: what a raise costs
    /// the frame thread from a cache file, for a 2048x2048 chain (the
    /// largest the test device allows; a level 0 of 16 MiB), against what
    /// the old path paid to read the whole chain synchronously. Run with
    /// `cargo test --release -p bsengine-rhi-wgpu --lib -- --ignored measure_raise --nocapture`;
    /// a debug build's numbers say nothing (validation layers, unoptimised
    /// copies).
    #[test]
    #[ignore]
    fn measure_raise_cost_from_a_cache_file() {
        use std::time::Instant;
        let (mut reg, dir) = make_registry_with_cache("measure");
        let pixels = vec![120u8; 2048 * 2048 * 4];
        let id = reg.load_with(2048, 2048, &pixels, streamed());
        drop(pixels);

        // The old path, for scale: every level from the base down, read
        // whole on the frame thread -- which is what a raise to base 0 did.
        let t = Instant::now();
        let whole = reg.textures[&id]
            .streamed
            .as_ref()
            .unwrap()
            .levels_from(0)
            .unwrap();
        let read_whole = t.elapsed();
        let whole_bytes: usize = whole.iter().map(|l| l.2.len()).sum();
        drop(whole);

        // The new path, level by level up to full residency.
        let mut rows = Vec::new();
        while reg.residency(id).unwrap().0 > 0 {
            let base = reg.residency(id).unwrap().0 - 1;
            let t = Instant::now();
            assert!(reg.raise_residency(id));
            let request = t.elapsed();
            let waited = Instant::now();
            let landing = loop {
                let t = Instant::now();
                let landed = reg.poll_reads();
                let poll = t.elapsed();
                if !landed.is_empty() {
                    break poll;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            };
            rows.push((base, request, waited.elapsed(), landing));
        }
        println!(
            "old path: reading the whole chain ({whole_bytes} bytes) on the frame thread: {read_whole:?}"
        );
        for (base, request, wait, landing) in rows {
            let (w, h) = reg.textures[&id].streamed.as_ref().unwrap().dims[base as usize];
            println!(
                "level {base} ({w}x{h}): request on the frame thread {request:?}, arrived after \
                 {wait:?}, landing (upload + GPU copies) on the frame thread {landing:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A cache directory that cannot be created -- here, a path that is a
    /// file -- is reported and the chain stays in RAM: streaming keeps
    /// working exactly as without a cache, only the memory is spent. A file
    /// of another layout under the right name is rewritten, not trusted.
    #[test]
    fn an_unusable_cache_dir_falls_back_to_memory_and_a_bad_file_is_rewritten() {
        let blocker =
            std::env::temp_dir().join(format!("bse_mip_cache_blocker_{}", std::process::id()));
        std::fs::write(&blocker, b"not a directory").unwrap();
        let mut reg = make_registry();
        reg.set_mip_cache_root(Some(blocker.join("mips")));
        let pixels = vec![3u8; 256 * 256 * 4];
        let id = reg.load_with(256, 256, &pixels, streamed());
        assert_eq!(reg.mip_cache_file(id), None, "no file could be written");
        assert!(
            reg.chain_ram_bytes(id).unwrap() > 0,
            "so the chain is kept in RAM"
        );
        assert!(reg.raise_residency(id) && reg.raise_residency(id));
        assert_eq!(
            reg.residency(id),
            Some((0, 9)),
            "and streaming works from it"
        );
        let _ = std::fs::remove_file(&blocker);

        // A complete, well-formed file of a *previous layout version* under
        // the right name: only the magic tells it apart, and it must be
        // rewritten, not read as if its bytes meant what this version's do.
        let (mut reg, dir) = make_registry_with_cache("bad_file");
        std::fs::create_dir_all(&dir).unwrap();
        let name = mip_cache_name(256, 256, &pixels, wgpu::TextureFormat::Rgba8UnormSrgb);
        let (mut stale, _) = encode_mip_cache(
            &mip_chain(256, 256, &pixels),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        stale[..8].copy_from_slice(b"BSMIPS00");
        std::fs::write(dir.join(&name), &stale).unwrap();
        let id = reg.load_with(256, 256, &pixels, streamed());
        assert_eq!(reg.mip_cache_file(id), Some(dir.join(&name).as_path()));
        let rewritten = std::fs::read(dir.join(&name)).unwrap();
        assert_eq!(
            &rewritten[..8],
            MIP_CACHE_MAGIC,
            "the stale file was rewritten in this version's layout"
        );
        let dims: Vec<(u32, u32)> = (0..9).map(|i| (256 >> i, 256 >> i)).collect();
        assert!(
            read_mip_cache_table(&dir.join(&name), &dims, wgpu::TextureFormat::Rgba8UnormSrgb)
                .is_some()
        );
        assert!(reg.raise_residency(id));
        assert_eq!(wait_for_read(&mut reg, id), vec![id]);
        assert_eq!(reg.residency(id), Some((1, 9)));

        // And a file too short to hold a header at all.
        std::fs::write(dir.join(&name), b"BSMIPS02 garbage").unwrap();
        let again = reg.load_with(256, 256, &pixels, streamed());
        assert_eq!(reg.mip_cache_file(again), Some(dir.join(&name).as_path()));
        assert!(
            read_mip_cache_table(&dir.join(&name), &dims, wgpu::TextureFormat::Rgba8UnormSrgb)
                .is_some(),
            "rewritten"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The header's encoding tag is checked on its own, not only through
    /// the level lengths: a file whose levels are BC1-sized but whose tag
    /// says another encoding is not read as BC1. The lengths would pass it
    /// -- which is the case a future 16-bytes-a-block encoding (BC7) would
    /// present against BC3 -- so this is what makes the tag a check at all.
    #[test]
    fn a_cache_file_is_read_only_as_the_encoding_tag_says() {
        use std::borrow::Cow;
        let dir = std::env::temp_dir().join("bsengine-mips-encoding_tag");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bc1 = wgpu::TextureFormat::Bc1RgbaUnormSrgb;
        let dims = [(8u32, 8u32), (4, 4), (2, 2), (1, 1)];
        let levels: Vec<(u32, u32, Cow<'_, [u8]>)> = dims
            .iter()
            .map(|&(w, h)| {
                let len = crate::profiler::level_bytes(bc1, w, h) as usize;
                (w, h, Cow::Owned(vec![0u8; len]))
            })
            .collect();
        let (bytes, _) = encode_mip_cache(&levels, bc1);
        let path = dir.join("tagged.mips");
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            read_mip_cache_table(&path, &dims, bc1).is_some(),
            "premise: as written, the file reads as BC1"
        );

        let mut retagged = bytes.clone();
        retagged[12..16]
            .copy_from_slice(&encoding_tag(wgpu::TextureFormat::Bc3RgbaUnormSrgb).to_le_bytes());
        std::fs::write(&path, &retagged).unwrap();
        assert!(
            read_mip_cache_table(&path, &dims, bc1).is_none(),
            "the same levels under a BC3 tag are not read as BC1, though every length fits"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A 64x64 PNG with more than one colour, and its decoded pixels as the
    /// texture loader would hand them over.
    fn png_fixture() -> (Vec<u8>, Vec<u8>) {
        let mut img = image::RgbaImage::new(64, 64);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([(x * 4) as u8, (y * 4) as u8, ((x ^ y) * 4) as u8, 255]);
        }
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let rgba = image::load_from_memory(&png).unwrap().to_rgba8().into_raw();
        (png, rgba)
    }

    /// The packager's file is the runtime's file: `precook_mip_cache` on the
    /// image's bytes names and fills exactly the cache file the registry
    /// writes the first time it uploads the decoded pixels. A different name
    /// would ship a file no runtime ever asks for; different bytes would
    /// ship a chain nobody encoded the way the runtime would.
    #[test]
    fn a_precooked_file_is_byte_for_byte_the_one_the_registry_writes() {
        use bsengine_core::TextureCompression;
        let (png, rgba) = png_fixture();
        let settings = TextureImportSettings {
            compression: TextureCompression::Bc1,
            ..Default::default()
        };
        let (name, bytes) = precook_mip_cache(&png, settings).expect("a BC1 texture precooks");

        let (mut reg, dir) = make_registry_with_cache("precooked_matches");
        assert!(
            reg.bc_supported(),
            "premise: block compression on this device"
        );
        reg.load_with(64, 64, &rgba, settings);
        assert_eq!(reg.encodes(), 1, "premise: the registry encoded it itself");
        let files = mips_files(&dir);
        assert_eq!(files.len(), 1, "{files:?}");
        assert_eq!(files[0].file_name().unwrap().to_str().unwrap(), name);
        assert_eq!(std::fs::read(&files[0]).unwrap(), bytes, "same bytes");
        let _ = std::fs::remove_dir_all(&dir);

        // Nothing to precook: uncompressed, or not whole blocks (the
        // registry uploads that one as RGBA8 and never asks).
        assert!(precook_mip_cache(&png, TextureImportSettings::default()).is_none());
        let mut odd = Vec::new();
        image::RgbaImage::new(66, 40)
            .write_to(&mut std::io::Cursor::new(&mut odd), image::ImageFormat::Png)
            .unwrap();
        assert!(precook_mip_cache(&odd, settings).is_none());
    }

    /// With a shipped file for the texture, the registry encodes nothing --
    /// even with no writable cache directory at all, which is a read-only
    /// install -- and uploads the shipped blocks. Without one it encodes, as
    /// before: the premise that `encodes() == 0` is not simply how this
    /// registry behaves.
    #[test]
    fn a_shipped_mip_cache_file_means_nothing_is_encoded_on_the_players_machine() {
        use bsengine_core::TextureCompression;
        let (png, rgba) = png_fixture();
        let settings = TextureImportSettings {
            compression: TextureCompression::Bc1,
            ..Default::default()
        };
        let (name, bytes) = precook_mip_cache(&png, settings).unwrap();

        let mut without = make_registry();
        without.set_shipped_mip_cache(Some(Arc::new(|_: &str| None)));
        without.load_with(64, 64, &rgba, settings);
        assert_eq!(
            without.encodes(),
            1,
            "premise: nothing shipped, so it encodes"
        );

        let asked = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let log = Arc::clone(&asked);
        let mut reg = make_registry();
        assert!(
            reg.mip_cache_root().is_none(),
            "premise: no cache directory"
        );
        reg.set_shipped_mip_cache(Some(Arc::new(move |n: &str| {
            log.lock().unwrap().push(n.to_string());
            (n == name).then(|| bytes.clone())
        })));
        let id = reg.load_with(64, 64, &rgba, settings);
        assert_eq!(
            reg.encodes(),
            0,
            "the shipped chain was used, not re-encoded"
        );
        assert_eq!(asked.lock().unwrap().len(), 1, "looked up once");
        assert_eq!(
            reg.get_gpu_shape(id),
            Some((wgpu::TextureFormat::Bc1RgbaUnormSrgb, 7)),
            "and it went up compressed, whole chain"
        );
    }

    #[test]
    fn get_unknown_id_returns_none() {
        let reg = make_registry();
        assert!(reg.get_bind_group(999).is_none());
    }

    #[test]
    fn load_invalid_bytes_returns_err() {
        let mut reg = make_registry();
        assert!(reg.load_from_bytes(b"not an image").is_err());
    }

    #[test]
    fn replace_swaps_texture_contents_under_the_same_id() {
        let mut reg = make_registry();

        let id = reg.load_from_rgba(1, 1, &[255, 0, 0, 255]);
        assert!(reg.get_bind_group(id).is_some());

        assert!(
            reg.replace(id, 2, 2, &[0u8; 16]),
            "replace must report success for an id that exists"
        );
        assert!(
            reg.get_bind_group(id).is_some(),
            "the id must still resolve after replace -- Material.texture_id \
             stores it and is not rewritten"
        );
        assert_eq!(
            reg.get_size(id),
            Some((2, 2)),
            "replace must actually rebuild the texture with the new dimensions \
             -- a no-op that returns true would leave the old 1x1 texture in place"
        );
    }

    #[test]
    fn replace_reports_failure_for_an_unknown_texture_id() {
        let mut reg = make_registry();
        assert!(!reg.replace(9999, 1, 1, &[0, 0, 0, 255]));
        assert!(reg.get_bind_group(9999).is_none());
    }

    #[test]
    fn get_view_resolves_a_loaded_texture_and_none_for_unknown() {
        let mut reg = make_registry();
        assert!(reg.get_view(999).is_none(), "unknown id must return None");

        let id = reg.load_from_rgba(2, 2, &[0u8; 16]);
        assert!(
            reg.get_view(id).is_some(),
            "a loaded texture's view must resolve"
        );
    }

    /// The two settings the GPU object itself records. `srgb` picks the
    /// format and `mipmaps` the level count; each is asserted against its
    /// opposite so a build that ignored the setting fails on one side.
    #[test]
    fn srgb_and_mipmaps_become_the_textures_format_and_level_count() {
        let mut reg = make_registry();
        let pixels = [128u8; 8 * 4 * 4];

        let colour = reg.load_with(8, 4, &pixels, TextureImportSettings::default());
        assert_eq!(
            reg.get_gpu_shape(colour),
            Some((wgpu::TextureFormat::Rgba8UnormSrgb, 4)),
            "sRGB format, and 8x4 has levels 8x4, 4x2, 2x1, 1x1"
        );

        let data = reg.load_with(
            8,
            4,
            &pixels,
            TextureImportSettings {
                srgb: false,
                mipmaps: false,
                ..Default::default()
            },
        );
        assert_eq!(
            reg.get_gpu_shape(data),
            Some((wgpu::TextureFormat::Rgba8Unorm, 1))
        );

        let raw = reg.load_from_rgba(8, 4, &pixels);
        assert_eq!(
            reg.get_settings(raw),
            Some(TextureImportSettings::raw()),
            "the settings-less path is the old upload, not the new default"
        );
        assert_eq!(
            reg.get_gpu_shape(raw),
            Some((wgpu::TextureFormat::Rgba8Unorm, 1))
        );
    }

    /// `replace` keeps the settings the texture was uploaded with;
    /// `replace_with` changes them. A reload that silently dropped a texture
    /// back to `raw` would undo its sidecar on the first edit of its pixels.
    #[test]
    fn replace_keeps_import_settings_and_replace_with_changes_them() {
        let mut reg = make_registry();
        let id = reg.load_with(2, 2, &[0u8; 16], TextureImportSettings::default());

        assert!(reg.replace(id, 1, 1, &[0, 0, 0, 255]));
        assert_eq!(reg.get_settings(id), Some(TextureImportSettings::default()));
        assert_eq!(
            reg.get_gpu_shape(id),
            Some((wgpu::TextureFormat::Rgba8UnormSrgb, 1)),
            "still sRGB; a 1x1 image has one level whatever `mipmaps` says"
        );

        let data = TextureImportSettings {
            srgb: false,
            ..Default::default()
        };
        assert!(reg.replace_with(id, 2, 2, &[0u8; 16], data));
        assert_eq!(reg.get_settings(id), Some(data));
        assert_eq!(
            reg.get_gpu_shape(id),
            Some((wgpu::TextureFormat::Rgba8Unorm, 2))
        );
    }

    /// The box filter, on numbers small enough to check by hand -- and on an
    /// odd width, where the last output texel averages the clamped edge.
    #[test]
    fn downsample_averages_two_by_two_blocks_and_clamps_odd_edges() {
        // 3x2 image: rows of [0, 100, 200] and [0, 100, 200] in R, opaque.
        let mut rgba = Vec::new();
        for _ in 0..2 {
            for r in [0u8, 100, 200] {
                rgba.extend_from_slice(&[r, 0, 0, 255]);
            }
        }
        let (w, h, out) = downsample(3, 2, &rgba);
        assert_eq!((w, h), (1, 1), "3x2 halves to 1x1 (3/2 = 1)");
        assert_eq!(out, vec![50, 0, 0, 255], "(0 + 100 + 0 + 100 + 2) / 4 = 50");

        let (w, h, out) = downsample(
            4,
            1,
            &[0, 0, 0, 255, 200, 0, 0, 255, 60, 0, 0, 255, 0, 0, 0, 255],
        );
        assert_eq!((w, h), (2, 1));
        assert_eq!(
            out[0], 100,
            "(0 + 200 + 0 + 200 + 2) / 4, the row clamped below itself"
        );
        assert_eq!(out[4], 30, "(60 + 0 + 60 + 0 + 2) / 4");

        let chain = mip_chain(5, 3, &[255u8; 5 * 3 * 4]);
        let sizes: Vec<(u32, u32)> = chain.iter().map(|(w, h, _)| (*w, *h)).collect();
        assert_eq!(sizes, vec![(5, 3), (2, 1), (1, 1)]);
    }

    fn streamed() -> TextureImportSettings {
        TextureImportSettings {
            srgb: false,
            mipmaps: true,
            streaming: true,
            ..Default::default()
        }
    }

    /// Bytes of an RGBA8 mip chain from a square power-of-two `top` level
    /// down to 1x1, as the profiler's geometric-series estimate counts it.
    fn chain_bytes(top: u32, levels: u32) -> u64 {
        let texels = (top * top) as f64 * (1.0 - 0.25f64.powi(levels as i32)) / 0.75;
        texels as u64 * 4
    }

    fn within_one_percent(actual: u64, expected: u64) -> bool {
        (actual as f64 - expected as f64).abs() <= expected as f64 * 0.01
    }

    /// The whole point of streaming, in bytes: a 256x256 streamed texture
    /// starts on the GPU as the 64x64 object holding levels 2..9 -- about
    /// 21 KiB where the full chain is 341 KiB -- and the profiler counts
    /// only that. Its logical size stays 256x256, because materials and UI
    /// lay out against the image, not against what is resident.
    #[test]
    fn a_streamed_texture_starts_with_only_its_small_levels_on_the_gpu() {
        let mut reg = make_registry();
        let pixels = vec![200u8; 256 * 256 * 4];

        let whole = reg.load_with(256, 256, &pixels, TextureImportSettings::default());
        let (_, _, whole_bytes) = reg.get_gpu_footprint(whole).unwrap();
        assert!(
            within_one_percent(whole_bytes, chain_bytes(256, 9)),
            "premise: the unstreamed chain is 9 levels of 256x256, {whole_bytes} bytes"
        );
        assert!(!reg.is_streaming(whole));
        assert_eq!(reg.residency(whole), None);

        let id = reg.load_with(256, 256, &pixels, streamed());
        assert!(reg.is_streaming(id));
        assert_eq!(
            reg.residency(id),
            Some((2, 9)),
            "levels 256, 128 wait; 64 is the first at or under {STREAMING_INITIAL_MAX_DIM}"
        );
        assert_eq!(reg.get_size(id), Some((256, 256)), "the logical size");
        let (w, h, bytes) = reg.get_gpu_footprint(id).unwrap();
        assert_eq!(
            (w, h),
            (64, 64),
            "the GPU object is the largest resident level"
        );
        assert_eq!(reg.get_gpu_shape(id).map(|s| s.1), Some(7), "levels 64..1");
        assert!(
            within_one_percent(bytes, chain_bytes(64, 7)),
            "and it costs only those levels: {bytes} bytes, not {whole_bytes}"
        );
        assert_eq!(reg.get_settings(id), Some(streamed()));
    }

    /// Each raise brings in exactly the next larger level -- the GPU object
    /// grows by that level's bytes and nothing else -- until level 0, after
    /// which raising reports nothing to do. Lowering is the reverse, and
    /// stops at the smallest level so the texture never becomes nothing.
    #[test]
    fn raising_and_lowering_residency_move_one_level_at_a_time() {
        let mut reg = make_registry();
        let id = reg.load_with(256, 256, &vec![9u8; 256 * 256 * 4], streamed());
        let bytes = |reg: &GpuTextureRegistry| reg.get_gpu_footprint(id).unwrap().2;
        let at_64 = bytes(&reg);

        assert!(reg.raise_residency(id));
        assert_eq!(reg.residency(id), Some((1, 9)));
        assert_eq!(
            reg.get_gpu_footprint(id).map(|f| (f.0, f.1)),
            Some((128, 128))
        );
        assert_eq!(reg.get_gpu_shape(id).map(|s| s.1), Some(8));
        let at_128 = bytes(&reg);
        assert!(
            within_one_percent(at_128 - at_64, 128 * 128 * 4),
            "one raise adds the 128x128 level and nothing else: {at_64} -> {at_128}"
        );

        assert!(reg.raise_residency(id));
        assert_eq!(reg.residency(id), Some((0, 9)), "fully resident");
        let at_256 = bytes(&reg);
        assert!(within_one_percent(at_256 - at_128, 256 * 256 * 4));
        assert!(
            !reg.raise_residency(id),
            "nothing above level 0 to bring in"
        );
        assert_eq!(reg.residency(id), Some((0, 9)));
        assert_eq!(bytes(&reg), at_256, "a refused raise must not rebuild");

        assert!(reg.lower_residency(id));
        assert_eq!(reg.residency(id), Some((1, 9)));
        assert_eq!(bytes(&reg), at_128, "lowering gives the level's bytes back");

        for _ in 0..7 {
            assert!(reg.lower_residency(id));
        }
        assert_eq!(reg.residency(id), Some((8, 9)), "down to the 1x1 level");
        assert!(!reg.lower_residency(id), "which always stays");
        assert_eq!(reg.get_gpu_footprint(id).map(|f| (f.0, f.1)), Some((1, 1)));

        // Neither call means anything for a texture that is not streamed.
        let plain = reg.load_with(8, 8, &[0u8; 8 * 8 * 4], TextureImportSettings::default());
        assert!(!reg.raise_residency(plain));
        assert!(!reg.lower_residency(plain));
        assert!(!reg.raise_residency(4242), "nor for an id nobody holds");
    }

    /// `streaming` without `mipmaps` has nothing to stream: one level is
    /// uploaded whole and the texture is not streamed, rather than being a
    /// streamed texture that can never change. And an image already at or
    /// under the initial size starts fully resident, so nothing is pending.
    #[test]
    fn streaming_needs_a_chain_and_a_small_image_is_resident_from_the_start() {
        let mut reg = make_registry();
        let unmipped = reg.load_with(
            256,
            256,
            &vec![0u8; 256 * 256 * 4],
            TextureImportSettings {
                mipmaps: false,
                ..streamed()
            },
        );
        assert!(!reg.is_streaming(unmipped));
        assert_eq!(reg.get_gpu_shape(unmipped).map(|s| s.1), Some(1));
        assert_eq!(reg.get_gpu_footprint(unmipped).map(|f| f.0), Some(256));

        let small = reg.load_with(64, 32, &[0u8; 64 * 32 * 4], streamed());
        assert!(reg.is_streaming(small), "streamed, with nothing waiting");
        assert_eq!(reg.residency(small), Some((0, 7)));
        assert!(!reg.raise_residency(small));
        assert_eq!(reg.raise_next_pending(), None);
    }

    /// The progressive loader's step: one level, on the streamed texture
    /// that has waited longest, so two textures brought up together each
    /// get a level in turn rather than one finishing before the other
    /// starts. Textures that are not streamed, or are already whole, are
    /// never picked, and the call says so once nothing is left.
    #[test]
    fn raise_next_pending_takes_turns_over_the_waiting_textures() {
        let mut reg = make_registry();
        let plain = reg.load_with(
            256,
            256,
            &vec![0u8; 256 * 256 * 4],
            TextureImportSettings::default(),
        );
        let a = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        let b = reg.load_with(128, 128, &vec![0u8; 128 * 128 * 4], streamed());
        assert_eq!(
            (reg.residency(a), reg.residency(b)),
            (Some((2, 9)), Some((1, 8))),
            "premise: a needs two raises, b one"
        );

        let order: Vec<Option<u64>> = (0..4).map(|_| reg.raise_next_pending()).collect();
        assert_eq!(
            order,
            vec![Some(a), Some(b), Some(a), None],
            "a, then b, then a's last level, then nothing"
        );
        assert_eq!(reg.residency(a), Some((0, 9)));
        assert_eq!(reg.residency(b), Some((0, 8)));
        assert_eq!(
            reg.get_gpu_footprint(plain).map(|f| f.0),
            Some(256),
            "the unstreamed texture was never touched"
        );
    }

    /// The level a screen size asks for: the smallest level at least that
    /// many pixels across, so a texture drawn 100 px tall wants its 128
    /// level and not its 256; anything at or over level 0's size wants
    /// level 0; the bias moves the answer along the chain and stops at
    /// its ends.
    #[test]
    fn the_wanted_level_is_the_smallest_that_covers_the_screen_size() {
        let chain = mip_chain(256, 256, &[0u8; 256 * 256 * 4]);
        let levels: Vec<(u32, u32)> = chain.iter().map(|(w, h, _)| (*w, *h)).collect();
        let want = |pixels: f32, bias: i32| wanted_base_for(&levels, pixels, bias);
        assert_eq!(want(100.0, 0), 1, "128 covers 100; 64 does not");
        assert_eq!(want(128.0, 0), 1, "exactly 128 is still the 128 level");
        assert_eq!(want(129.0, 0), 0);
        assert_eq!(want(300.0, 0), 0, "more than level 0 is still level 0");
        assert_eq!(want(f32::INFINITY, 0), 0);
        assert_eq!(want(10.0, 0), 4, "16 covers 10");
        assert_eq!(want(1.0, 0), 8, "the 1x1 level");
        assert_eq!(want(0.0, 0), 8);
        assert_eq!(want(100.0, 1), 2, "one level smaller than asked");
        assert_eq!(want(100.0, -1), 0);
        assert_eq!(want(300.0, -3), 0, "clamped at the top");
        assert_eq!(want(1.0, 5), 8, "and at the bottom");

        // A non-square chain is measured by its larger side.
        let chain = mip_chain(256, 64, &[0u8; 256 * 64 * 4]);
        let levels: Vec<(u32, u32)> = chain.iter().map(|(w, h, _)| (*w, *h)).collect();
        assert_eq!(wanted_base_for(&levels, 100.0, 0), 1);
    }

    /// Wants steer the step: a texture drawn small settles at the level its
    /// screen size asks for and no higher, the texture furthest below its
    /// want is served first, a texture that comes to want less gives its
    /// levels back one per step, and one level of slack keeps a want that
    /// moves by one from rebuilding anything. A streamed texture nothing
    /// measured is wanted whole, as before.
    #[test]
    fn step_streaming_moves_each_texture_towards_its_want() {
        let mut reg = make_registry();
        let a = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        let b = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        let c = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        assert_eq!(reg.residency(a), Some((2, 9)), "premise: 64 resident");

        // a is drawn 300 px tall (wants everything), b 40 px (wants 64:
        // exactly what it holds), c is not drawn at all.
        let wants = HashMap::from([(a, 300.0), (b, 40.0)]);
        reg.set_wants(&wants, 0);
        assert_eq!(
            (reg.wanted(a), reg.wanted(b), reg.wanted(c)),
            (Some(0), Some(2), Some(0))
        );
        assert_eq!(reg.textures_below_wanted(), 2, "a and c");

        // a and c both want two more levels; they take turns, b is never
        // touched, and the step reports nothing once both are whole.
        let steps: Vec<Option<StreamingStep>> = (0..5).map(|_| reg.step_streaming(None)).collect();
        assert_eq!(
            steps,
            vec![
                Some(StreamingStep::Raised(a)),
                Some(StreamingStep::Raised(c)),
                Some(StreamingStep::Raised(a)),
                Some(StreamingStep::Raised(c)),
                None
            ]
        );
        assert_eq!(reg.residency(a), Some((0, 9)));
        assert_eq!(reg.residency(c), Some((0, 9)));
        assert_eq!(reg.residency(b), Some((2, 9)), "b holds what it wants");
        assert_eq!(reg.textures_below_wanted(), 0);

        // a moves away: drawn 10 px, it wants the 16 level (index 4). It
        // gives levels back one per step and stops one short (index 3) --
        // the slack that stops a want flickering across a boundary from
        // rebuilding every frame.
        reg.set_wants(&HashMap::from([(a, 10.0), (b, 40.0), (c, 300.0)]), 0);
        assert_eq!(reg.wanted(a), Some(4));
        let steps: Vec<Option<StreamingStep>> = (0..5).map(|_| reg.step_streaming(None)).collect();
        assert_eq!(
            steps,
            vec![
                Some(StreamingStep::Lowered(a)),
                Some(StreamingStep::Lowered(a)),
                Some(StreamingStep::Lowered(a)),
                None,
                None
            ]
        );
        assert_eq!(reg.residency(a), Some((3, 9)));

        // Wanting exactly one level less than held changes nothing.
        reg.set_wants(&HashMap::from([(a, 10.0), (b, 20.0), (c, 300.0)]), 0);
        assert_eq!(reg.wanted(b), Some(3), "b holds 2, wants 3");
        assert_eq!(reg.step_streaming(None), None);
        assert_eq!(reg.residency(b), Some((2, 9)));

        // A raise takes precedence over a lowering when both are due.
        reg.set_wants(&HashMap::from([(a, 300.0), (b, 5.0), (c, 300.0)]), 0);
        assert_eq!(reg.step_streaming(None), Some(StreamingStep::Raised(a)));
    }

    /// The budget wins over wants, as Unreal's pool does: a level that would
    /// exceed it is not brought in; over it, the texture holding the most it
    /// does not want gives a level back first, and when nothing has a
    /// surplus the largest resident level goes; a budget of `None` limits
    /// nothing. The bias is applied to every want.
    #[test]
    fn the_budget_caps_residency_and_evicts_the_least_wanted_first() {
        let mut reg = make_registry();
        let a = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        let b = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        let at_64 = reg.streamed_resident_bytes();
        assert!(
            within_one_percent(at_64, 2 * chain_bytes(64, 7)),
            "premise: two 64-level chains, {at_64} bytes"
        );

        // Both want everything, but the budget has room for one 128 level
        // and not two: a is raised, b is refused, and nothing is lowered
        // because nothing holds more than it wants.
        reg.set_wants(&HashMap::from([(a, 300.0), (b, 300.0)]), 0);
        let budget = at_64 + 128 * 128 * 4 + 1024;
        assert_eq!(
            reg.step_streaming(Some(budget)),
            Some(StreamingStep::Raised(a))
        );
        assert_eq!(
            reg.step_streaming(Some(budget)),
            None,
            "b's 128 level does not fit"
        );
        assert_eq!(
            (reg.residency(a), reg.residency(b)),
            (Some((1, 9)), Some((2, 9)))
        );
        assert_eq!(reg.textures_below_wanted(), 2, "both are held down");

        // Now a wants less (drawn 40 px: the 64 level) and b still wants
        // everything: a's surplus level goes, which makes room for b next.
        reg.set_wants(&HashMap::from([(a, 40.0), (b, 300.0)]), 0);
        assert_eq!(
            reg.step_streaming(Some(budget)),
            Some(StreamingStep::Lowered(a))
        );
        assert_eq!(
            reg.step_streaming(Some(budget)),
            Some(StreamingStep::Raised(b))
        );
        assert_eq!(
            (reg.residency(a), reg.residency(b)),
            (Some((2, 9)), Some((1, 9)))
        );

        // The budget shrinks below what is held: b, holding a level it
        // wants, is still the one with the largest resident level once a
        // has no surplus, so b gives it back.
        let tiny = Some(at_64);
        assert_eq!(reg.step_streaming(tiny), Some(StreamingStep::Lowered(b)));
        assert_eq!(reg.residency(b), Some((2, 9)));
        assert!(reg.streamed_resident_bytes() <= at_64);
        // Still over? No: exactly at. Nothing more is dropped, and b is not
        // raised either since its level would not fit.
        assert_eq!(reg.step_streaming(tiny), None);

        // The bias: with +1 every want is one level smaller, so b, drawn
        // 300 px, wants the 128 level and stops there under no budget.
        reg.set_wants(&HashMap::from([(a, 40.0), (b, 300.0)]), 1);
        assert_eq!((reg.wanted(a), reg.wanted(b)), (Some(3), Some(1)));
        assert_eq!(reg.step_streaming(None), Some(StreamingStep::Raised(b)));
        assert_eq!(reg.step_streaming(None), None);
        assert_eq!(reg.residency(b), Some((1, 9)));

        // Eviction order: b whole and wanted whole, a holding one level it
        // does not want. Over budget, a's unwanted level goes before b's
        // larger one -- the least wanted first, not the largest.
        reg.set_wants(&HashMap::from([(a, 20.0), (b, 300.0)]), 0);
        assert_eq!(
            (reg.residency(a), reg.wanted(a)),
            (Some((2, 9)), Some(3)),
            "premise: a holds 64 and wants 32"
        );
        assert_eq!(reg.step_streaming(None), Some(StreamingStep::Raised(b)));
        assert_eq!(
            reg.step_streaming(None),
            None,
            "a's one level of surplus is within the slack"
        );
        let over = Some(reg.streamed_resident_bytes() - 1);
        assert_eq!(
            reg.step_streaming(over),
            Some(StreamingStep::Lowered(a)),
            "the least wanted level goes, not the largest"
        );
        assert_eq!(
            (reg.residency(a), reg.residency(b)),
            (Some((3, 9)), Some((0, 9)))
        );

        // Two textures both past their want: the one further past goes
        // first even though the other's level is far larger -- a's 32
        // level (two past) before b's 256 (one past). A filter alone cannot
        // tell these apart; the ordering can.
        reg.set_wants(&HashMap::from([(a, 8.0), (b, 128.0)]), 0);
        assert_eq!(
            (reg.wanted(a), reg.wanted(b)),
            (Some(5), Some(1)),
            "premise: a holds 3 and wants 5, b holds 0 and wants 1"
        );
        let over = Some(reg.streamed_resident_bytes() - 1);
        assert_eq!(
            reg.step_streaming(over),
            Some(StreamingStep::Lowered(a)),
            "the furthest past its want goes first, not the largest"
        );
        assert_eq!(
            (reg.residency(a), reg.residency(b)),
            (Some((4, 9)), Some((0, 9)))
        );

        // A texture that is not streamed is never in any of this.
        let plain = reg.load_with(8, 8, &[0u8; 8 * 8 * 4], TextureImportSettings::default());
        assert_eq!(reg.wanted(plain), None);
        assert_eq!(
            reg.step_streaming(Some(1)),
            Some(StreamingStep::Lowered(b)),
            "over a budget of 1 byte, b (largest) goes"
        );
    }

    /// A reload -- new pixels, or a sidecar edit -- rebuilds a streamed
    /// texture at its initial residency, and turning `streaming` off on
    /// reload uploads it whole: what is resident follows the settings in
    /// force, not the history of raises.
    #[test]
    fn replacing_a_streamed_texture_restarts_it_and_can_stop_streaming_it() {
        let mut reg = make_registry();
        let id = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        assert!(reg.raise_residency(id) && reg.raise_residency(id));
        assert_eq!(reg.residency(id), Some((0, 9)), "premise: fully resident");

        assert!(reg.replace(id, 256, 256, &vec![7u8; 256 * 256 * 4]));
        assert_eq!(
            reg.residency(id),
            Some((2, 9)),
            "back to the initial levels"
        );
        assert_eq!(reg.get_gpu_footprint(id).map(|f| f.0), Some(64));

        assert!(reg.replace_with(
            id,
            256,
            256,
            &vec![7u8; 256 * 256 * 4],
            TextureImportSettings::default()
        ));
        assert!(!reg.is_streaming(id));
        assert_eq!(reg.get_gpu_footprint(id).map(|f| f.0), Some(256));
    }
}
