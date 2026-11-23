//! The GLSL ES gate: every WGSL module this crate makes must translate to
//! GLSL ES 3.10, the shading language of the oldest GLES this renderer runs on.
//!
//! On a GLES device wgpu hands each WGSL module to naga's GLSL backend when
//! the pipeline is made, and a module naga cannot translate is an invalid
//! pipeline -- for the mesh shader, every frame. No CI runner has a GLES
//! device, and Vulkan, D3D12 and Metal translate the same WGSL without
//! complaint, so nothing else in the suite sees it: the first GLES run of
//! this renderer found thirteen shaders that read depth with `textureLoad`
//! from a `texture_depth_2d`, which naga's GLSL backend refuses ("WGSL
//! `textureLoad` from depth textures is not supported in GLSL"), all green
//! on every other backend.
//!
//! So this test runs naga's GLSL ES writer over every module the crate
//! recorded (`pipeline_cache::RECORDED_WGSL`, which sees every module because
//! `clippy.toml` sends them all through one constructor), on whatever device
//! the test machine has.
//!
//! What it does not cover: limits and capabilities that only a GLES driver
//! reports (cube-map arrays, independent blending, storage-buffer counts).
//! Those are adapter capabilities the renderer checks at run time, not
//! properties of the WGSL text.

#[cfg(test)]
mod tests {
    use crate::surface::WgpuSurface;

    /// Modules known not to translate yet, each with the error it gives:
    /// GLES support is landing a PR at a time, and these are the ones still
    /// to come. An entry that starts translating fails the test too, so the
    /// list only ever shrinks.
    const KNOWN_FAILURES: &[(&str, &str)] = &[];

    /// Labels in the source the gate does not make, and why.
    const NOT_ENGINE_SHADERS: &[(&str, &str)] = &[
        (
            "custom shader",
            "a game's own WGSL, compiled from its project",
        ),
        (
            "wgsl shader",
            "`WgpuSurface::compile_shader`, which checks WGSL it is handed",
        ),
        (
            "probe cells",
            "`probe_cells_for_testing`'s, a test helper's compute pass",
        ),
    ];

    /// Every entry point of `source`, translated to GLSL ES 3.10; the errors.
    fn glsl_es_errors(source: &str) -> Vec<String> {
        let module = match naga::front::wgsl::parse_str(source) {
            Ok(m) => m,
            Err(e) => return vec![format!("parse: {e}")],
        };
        let info = match naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        {
            Ok(i) => i,
            Err(e) => return vec![format!("validate: {e:?}")],
        };
        let options = naga::back::glsl::Options {
            version: naga::back::glsl::Version::Embedded {
                version: 310,
                is_webgl: false,
            },
            ..Default::default()
        };
        let mut errors = Vec::new();
        for ep in &module.entry_points {
            let pipeline_options = naga::back::glsl::PipelineOptions {
                shader_stage: ep.stage,
                entry_point: ep.name.clone(),
                multiview: None,
            };
            let mut out = String::new();
            let written = naga::back::glsl::Writer::new(
                &mut out,
                &module,
                &info,
                &options,
                &pipeline_options,
                naga::proc::BoundsCheckPolicies::default(),
            )
            .and_then(|mut w| w.write().map(|_| ()));
            if let Err(e) = written {
                errors.push(format!("{} ({:?}): {e}", ep.name, ep.stage));
            }
        }
        errors
    }

    /// The label of every module this crate's non-test code makes with a
    /// literal label -- what the gate must have seen. Read from the source,
    /// so a shader added later and made only on first use (as the ones
    /// [`make_every_module`] reaches for by hand are) fails the gate as
    /// "never made" instead of passing unchecked.
    fn labels_in_source() -> Vec<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut labels = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("src") {
            let path = entry.expect("entry").path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("source");
            // The module's own tests make one-off modules this gate does not
            // ask for; they sit after `#[cfg(test)] mod tests`.
            let text = text
                .split("\n#[cfg(test)]\nmod tests")
                .next()
                .unwrap_or_default();
            let mut rest = text;
            const DESC: &str = "ShaderModuleDescriptor {";
            while let Some(at) = rest.find(DESC) {
                rest = &rest[at + DESC.len()..];
                let after = rest.trim_start();
                if let Some(label) = after
                    .strip_prefix("label: Some(\"")
                    .and_then(|l| l.split('"').next())
                {
                    labels.push(label.to_string());
                }
            }
        }
        labels.sort();
        labels.dedup();
        labels
    }

    /// Makes every module this crate has: a renderer's own, then the ones a
    /// renderer makes only on first use, or that are not a renderer's.
    fn make_every_module() {
        // A renderer built as for an OpenGL ES adapter, too: its fallbacks
        // make shaders and pipelines of their own (the decal shader that
        // encodes sRGB itself), and those are the ones GLES actually gets.
        WgpuSurface::offscreen_for_testing_with_caps(64, 64, |caps| {
            caps.view_formats = false;
            caps.realtime_gi = false;
        })
        .expect("offscreen renderer as for GLES");
        let mut surface = WgpuSurface::offscreen_for_testing(64, 64).expect("offscreen renderer");
        surface.make_lazy_pipelines_for_gles_gate();
        surface.set_skybox_from_rgba(1, 1, &[128, 128, 255, 255]);
        let device = surface.device_arc();
        let queue = surface.queue_arc();
        crate::skinning::SkinningPipeline::new(&device);
        let (_, _, gi_ready) = crate::realtime_gi::build_pipelines_for_testing(&device);
        assert!(gi_ready, "the realtime GI pipelines were built");
        for format in [
            bsengine_core::HdrOutputFormat::Hdr10,
            bsengine_core::HdrOutputFormat::Scrgb,
        ] {
            WgpuSurface::offscreen_hdr_for_testing(
                64,
                64,
                bsengine_core::HdrOutputSettings {
                    enabled: true,
                    format,
                    ..Default::default()
                },
            )
            .expect("HDR offscreen renderer");
        }
        let triangle = crate::mesh_thumbnail::ThumbnailMesh {
            primitives: vec![crate::mesh_thumbnail::ThumbnailPrimitive {
                vertices: vec![bytemuck::Zeroable::zeroed(); 3],
                indices: vec![0, 1, 2],
                base_color_factor: [1.0; 4],
                base_color_texture: None,
            }],
        };
        crate::mesh_thumbnail::render_thumbnail(&device, &queue, &triangle);
        // A game's unlit custom shader: its pipelines are the renderer's
        // (`unlit_custom_desc`), only the WGSL is the game's. A lit one is
        // built with the opaque mesh pipeline's descriptor, which the
        // renderer above made already.
        surface
            .compile_and_store_shader(
                "gles gate unlit",
                "@vertex fn vs_main() -> @builtin(position) vec4<f32> {\n\
                     return vec4<f32>(0.0, 0.0, 0.0, 1.0);\n\
                 }\n\
                 @fragment fn fs_main() -> @location(0) vec4<f32> {\n\
                     return vec4<f32>(1.0);\n\
                 }\n",
            )
            .expect("the unlit custom shader validates");
        for (path, verdict) in surface.finish_custom_shaders() {
            verdict.unwrap_or_else(|e| panic!("custom shader {path}: {e}"));
        }
    }

    #[test]
    fn every_engine_shader_translates_to_glsl_es_3_1() {
        make_every_module();
        let recorded = crate::pipeline_cache::RECORDED_WGSL
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        // The premise: the gate saw every shader the source names, among
        // them the thirteen that read depth.
        let seen: Vec<&str> = recorded.iter().map(|(l, _)| l.as_str()).collect();
        let never_made: Vec<String> = labels_in_source()
            .into_iter()
            .filter(|l| !seen.contains(&l.as_str()))
            .filter(|l| !NOT_ENGINE_SHADERS.iter().any(|(n, _)| n == l))
            .collect();
        assert!(
            never_made.is_empty(),
            "shaders the source makes that the gate never saw made -- make them \
             in `make_every_module`: {never_made:?}"
        );
        for depth_reader in [
            "decal shader",
            "ssao shader",
            "taa shader",
            "gpu particles shader",
        ] {
            assert!(
                seen.contains(&depth_reader),
                "premise: {depth_reader} recorded"
            );
        }

        // The record is the process's: tests running beside this one add
        // their own modules to it. Those without a label are tests' one-offs,
        // and a `NOT_ENGINE_SHADERS` one can be WGSL that is invalid on
        // purpose -- a `compile_shader` test's ("expected `)`, found 1.0"
        // failed this gate in a full run and never alone).
        let engine =
            |label: &str| !label.is_empty() && !NOT_ENGINE_SHADERS.iter().any(|(n, _)| *n == label);
        let mut failures = Vec::new();
        for (label, source) in recorded.iter().filter(|(l, _)| engine(l)) {
            for error in glsl_es_errors(source) {
                failures.push((label.clone(), error));
            }
        }
        let unexpected: Vec<_> = failures
            .iter()
            .filter(|(label, error)| {
                !KNOWN_FAILURES
                    .iter()
                    .any(|(l, e)| l == label && error.contains(e))
            })
            .collect();
        let fixed: Vec<_> = KNOWN_FAILURES
            .iter()
            .filter(|(l, e)| {
                !failures
                    .iter()
                    .any(|(label, error)| label == l && error.contains(e))
            })
            .collect();
        assert!(
            unexpected.is_empty() && fixed.is_empty(),
            "{} modules checked.\nDo not translate to GLSL ES 3.10: {unexpected:#?}\n\
             Listed as failing but now translate (remove from KNOWN_FAILURES): {fixed:#?}",
            recorded.len()
        );
    }

    /// Pipelines that still give their colour targets different blend states
    /// or write masks -- each draws colour only and masks the other targets
    /// off. GLES 3.1 has no independent blending, so on it these are invalid
    /// until they get a pass of their own. Like `KNOWN_FAILURES`, an entry
    /// that stops mixing fails the test, so the list only shrinks.
    const KNOWN_MIXED_TARGETS: &[&str] = &[
        "transparent mesh pipeline",
        "terrain pipeline",
        "custom pipeline",
    ];

    /// Every render pipeline gives all its colour targets one blend state and
    /// write mask, which is all GLES 3.1 can do (`INDEPENDENT_BLEND` is core
    /// only from 3.2). wgpu checks this when a pipeline is made, on the
    /// adapter's flags, so on every backend CI has the test is the only thing
    /// that sees it.
    #[test]
    fn no_pipeline_needs_independent_blending() {
        make_every_module();
        let mixed = crate::pipeline_cache::MIXED_TARGET_PIPELINES
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        // The premise: what this test watches is recorded at all -- the
        // known ones are made, and do mix.
        for known in KNOWN_MIXED_TARGETS {
            assert!(
                mixed.iter().any(|m| m == known),
                "{known} is listed as mixing its targets but was not recorded doing so: \
                 if it no longer does, remove it from KNOWN_MIXED_TARGETS. Recorded: {mixed:?}"
            );
        }
        let unexpected: Vec<_> = mixed
            .iter()
            .filter(|m| !KNOWN_MIXED_TARGETS.contains(&m.as_str()))
            .collect();
        assert!(
            unexpected.is_empty(),
            "pipelines whose colour targets differ in blend or write mask, which GLES \
             3.1 cannot make (`REPLACE` and `None` count as different): {unexpected:?}"
        );
    }
}
