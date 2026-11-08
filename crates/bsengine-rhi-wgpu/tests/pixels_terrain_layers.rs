//! A terrain's layers as texture arrays (`terrain_layers`): each kind of
//! layer texture is copied into one array the shader indexes by layer,
//! instead of a binding per layer, which had the terrain at fifteen of
//! WebGPU's sixteen textures.
//!
//! The arrays are copies, so what these tests hold them to is what a copy can
//! get wrong: a layer landing in another layer's slot, one of the two ways of
//! filling an array drawing something the other does not, a copy that is not
//! redone when its source changes -- or is redone every frame -- and a copy
//! kept after nothing draws it.

mod common;

use bsengine_core::{TextureCompression, TextureImportSettings};
use common::{Harness, Pixels, Scene};
use glam::{Mat4, Vec3};

/// A `size` x `size` texture of one colour, with `settings`.
fn solid(h: &mut Harness, size: u32, rgba: [u8; 4], settings: TextureImportSettings) -> u64 {
    let texels: Vec<u8> = (0..size * size).flat_map(|_| rgba).collect();
    h.texture_with(settings, size, size, &texels)
}

/// Four clearly different layer colours.
const COLOURS: [[u8; 4]; 4] = [
    [200, 40, 40, 255],
    [40, 200, 40, 255],
    [40, 40, 200, 255],
    [200, 200, 40, 255],
];

/// A weight texture putting all of the chunk on `layer`.
fn all_on(h: &mut Harness, layer: usize) -> u64 {
    let mut rgba = [0u8; 4];
    rgba[layer] = 255;
    h.two_colour_texture(rgba, rgba)
}

/// A flat chunk filling the view, lit plainly from above.
fn render(h: &mut Harness, layers: [u64; 4], weight: u64) -> Pixels {
    let cube = h.cube();
    h.render(&Scene {
        terrain: vec![bsengine_rhi_wgpu::TerrainDrawCall::new(
            cube,
            Mat4::from_scale(Vec3::new(20.0, 1.0, 20.0)),
            layers,
            weight,
        )],
        camera_pos: Vec3::new(0.0, 6.5, 0.01),
        look_at: Vec3::new(0.0, 0.5, 0.0),
        ..Scene::default()
    })
}

/// The centre pixel with all of the chunk on each layer in turn.
fn each_layer(h: &mut Harness, layers: [u64; 4]) -> [[u8; 4]; 4] {
    std::array::from_fn(|l| {
        let weight = all_on(h, l);
        render(h, layers, weight).centre()
    })
}

/// Each layer shows its own texture, whichever way its array was filled:
/// copied, when the four layer textures share a format, size and level count,
/// or drawn, when they do not. Solid colours, so a layer drawn at another
/// size reads the same as one copied as it is, and any difference between the
/// two is a layer in the wrong slot or a fill that did not land.
#[test]
fn each_layer_shows_its_own_texture_copied_or_drawn() {
    let mut h = Harness::new();
    let settings = TextureImportSettings::default();
    let same_shape = COLOURS.map(|c| solid(&mut h, 8, c, settings));
    let sizes = [8, 4, 16, 2];
    let mixed: [u64; 4] = std::array::from_fn(|l| solid(&mut h, sizes[l], COLOURS[l], settings));

    let before = h.surface().terrain_layer_array_stats();
    let copied = each_layer(&mut h, same_shape);
    let after_copied = h.surface().terrain_layer_array_stats();
    let drawn = each_layer(&mut h, mixed);
    let after_drawn = h.surface().terrain_layer_array_stats();

    for a in 0..4 {
        for b in a + 1..4 {
            assert_ne!(
                copied[a], copied[b],
                "premise: layers {a} and {b} look different: {copied:?}"
            );
        }
    }
    // The colour array once, and the (empty) normal-map array once: four
    // frames with the same layers fill neither again.
    assert_eq!(
        (
            after_copied.copied - before.copied,
            after_copied.drawn - before.drawn
        ),
        (1, 1),
        "premise: one shape copies the colours; the normal maps, none, are drawn"
    );
    // The normal maps are the same four `None`s, so the same array.
    assert_eq!(
        (
            after_drawn.copied - after_copied.copied,
            after_drawn.drawn - after_copied.drawn
        ),
        (0, 1),
        "premise: four sizes draw the colours"
    );
    assert_eq!(
        copied, drawn,
        "a layer reads the same whether its array was copied or drawn"
    );
}

/// A block-compressed layer is copied as it is -- still compressed -- block
/// for block, down to the mip levels smaller than a block, whose copy is the
/// whole padded block.
#[test]
fn compressed_layers_are_copied_compressed() {
    let mut h = Harness::new();
    if !h.bc_supported() {
        eprintln!("skipped: this adapter does not sample BC textures");
        return;
    }
    let bc1 = TextureImportSettings {
        compression: TextureCompression::Bc1,
        ..Default::default()
    };
    let plain = COLOURS.map(|c| solid(&mut h, 8, c, TextureImportSettings::default()));
    let compressed = COLOURS.map(|c| solid(&mut h, 8, c, bc1));
    let (format, levels) = h.gpu_shape(compressed[0]).unwrap();
    assert!(
        format.is_compressed() && levels == 4,
        "premise: 8x8 with every level down to 1x1, compressed: {format:?}, {levels} levels"
    );

    let reference = each_layer(&mut h, plain);
    let before = h.surface().terrain_layer_array_stats();
    let from_compressed = each_layer(&mut h, compressed);
    let after = h.surface().terrain_layer_array_stats();

    assert_eq!(
        after.copied - before.copied,
        1,
        "the compressed colours were copied"
    );
    for l in 0..4 {
        let off = (0..3)
            .map(|c| reference[l][c].abs_diff(from_compressed[l][c]))
            .max()
            .unwrap();
        // BC1 keeps a solid colour to its 5:6:5 endpoints.
        assert!(
            off <= 8,
            "layer {l}: {:?} compressed, {:?} not",
            from_compressed[l],
            reference[l]
        );
    }
}

/// A layer texture reloaded -- a new object behind the same id -- is copied
/// again the next frame, and the terrain shows it. The other side matters as
/// much: a frame in which nothing changed copies nothing, or the arrays would
/// be refilled every frame, which is what they are not for.
#[test]
fn a_reloaded_layer_is_copied_again_and_nothing_else_is() {
    let mut h = Harness::new();
    let settings = TextureImportSettings::default();
    let layers = COLOURS.map(|c| solid(&mut h, 8, c, settings));
    let weight = all_on(&mut h, 0);

    let first = render(&mut h, layers, weight);
    let settled = h.surface().terrain_layer_array_stats();
    let again = render(&mut h, layers, weight);
    let unchanged = h.surface().terrain_layer_array_stats();
    assert_eq!(
        unchanged, settled,
        "a frame with nothing changed fills nothing"
    );
    assert!(!again.differs_from(&first), "and draws the same");

    let texels: Vec<u8> = (0..64).flat_map(|_| COLOURS[2]).collect();
    assert!(h.replace_texture(layers[0], settings, 8, 8, &texels));
    let reloaded = render(&mut h, layers, weight);
    let after = h.surface().terrain_layer_array_stats();

    assert_eq!(
        after.copied - unchanged.copied,
        1,
        "the reloaded layer's array is copied again"
    );
    let blue = all_on(&mut h, 2);
    assert_eq!(
        reloaded.centre(),
        render(&mut h, layers, blue).centre(),
        "layer 0 now shows what was loaded into it"
    );
    assert_ne!(
        reloaded.centre(),
        first.centre(),
        "premise: what was loaded looks different"
    );
}

/// An array nothing draws any more is dropped: a terrain removed from the
/// scene leaves no copies of its layers on the GPU.
#[test]
fn arrays_no_terrain_draws_are_dropped() {
    let mut h = Harness::new();
    let layers = COLOURS.map(|c| solid(&mut h, 8, c, TextureImportSettings::default()));
    let weight = all_on(&mut h, 0);

    render(&mut h, layers, weight);
    assert_eq!(
        h.surface().terrain_layer_array_stats().live,
        2,
        "premise: a drawn terrain holds its colours and its normal maps"
    );
    h.render(&Scene::default());
    assert_eq!(h.surface().terrain_layer_array_stats().live, 0);
}
