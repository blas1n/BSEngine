//! Cloth, simulated as a mesh of vertices held together by its own edges.
//!
//! Kept free of ECS types, the same way `terrain_chunking` is: the maths is
//! testable against a list of positions and a list of edges, with no entity to
//! spawn and no device to open. (`Vertex` is plain `#[repr(C)]` data, not a GPU
//! handle -- `terrain_chunking` builds them here too.)
//!
//! # Why position-based, and what the three engines do
//!
//! Unity's `Cloth`, Unreal's Chaos Cloth and Godot's `SoftBody3D` are all the
//! same construction underneath: the vertices are moved by gravity, then pulled
//! back toward their rest distances over a few iterations, and some of them are
//! pinned. Position-based dynamics is what makes that stable at a game's
//! timestep -- a force-based spring mesh stiff enough to look like cloth needs
//! a timestep far smaller than a frame, and explodes when it does not get one.
//!
//! All three also pin vertices, and that is not a refinement: a cloth with
//! nothing held falls out of the world on the first second. Godot spells it
//! `pinned_points`, Unity constrains vertices through its cloth constraints,
//! Unreal paints a max-distance map. This takes the list of indices, which is
//! the shape the other two reduce to.

use bsengine_rhi_wgpu::Vertex;
use glam::Vec3;

/// What a [`Link`] is resisting, and therefore which stiffness applies to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// An edge of the mesh. Resists the sheet being stretched or sheared, and
    /// is what makes it a fabric rather than a cloud of points.
    Structural,
    /// A span between two vertices with one between them. Resists the sheet
    /// *folding*, which its own edges cannot: three vertices in a row can hinge
    /// freely about the middle one without changing either edge's length.
    ///
    /// Unity spells this `bendingStiffness` and Unreal `BendingStiffness`;
    /// Godot has no equivalent. Both of those also offer a dihedral-angle
    /// formulation between adjacent triangles, which is more faithful and more
    /// expensive; the two-apart span is what game cloth normally uses and needs
    /// no solver of its own -- it is a distance constraint like any other.
    Bend,
}

/// The pair of vertices one distance constraint holds together, and the
/// distance it holds them at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Link {
    /// First vertex index.
    pub a: u32,
    /// Second vertex index.
    pub b: u32,
    /// Distance the two rest at, taken from the mesh before it was simulated.
    pub rest: f32,
    /// Which stiffness [`step`] should enforce this one with.
    pub kind: LinkKind,
}

/// Every distinct edge of a triangle mesh, with the length it starts at.
///
/// Edges, not triangles: an edge shared by two triangles must be one
/// constraint, not two, or it is enforced twice as hard as the mesh's border
/// edges and the cloth creases along its own topology.
pub fn links_from_indices(positions: &[Vec3], indices: &[u32]) -> Vec<Link> {
    let mut seen = std::collections::HashSet::new();
    let mut links = Vec::new();
    for tri in indices.chunks_exact(3) {
        for (x, y) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let (lo, hi) = if x <= y { (x, y) } else { (y, x) };
            if lo == hi || !seen.insert((lo, hi)) {
                continue;
            }
            let (Some(pa), Some(pb)) = (positions.get(lo as usize), positions.get(hi as usize))
            else {
                continue;
            };
            links.push(Link {
                a: lo,
                b: hi,
                rest: (*pb - *pa).length(),
                kind: LinkKind::Structural,
            });
        }
    }
    links
}

/// The spans that stop a grid folding: every pair of vertices two apart along a
/// row, and every pair two apart down a column.
///
/// Not derived from the triangles the way [`links_from_indices`] is, because
/// "two apart" is a statement about the grid's layout and the index list has
/// already forgotten it. Cloth generated any other way would need its own
/// version of this -- which is why it takes the dimensions rather than a mesh.
///
/// Diagonals are left out. The grid's triangulation already spans each cell
/// diagonally as a structural edge, and a sheet folding along a diagonal has to
/// bend a row or a column to do it.
pub fn bend_links(positions: &[Vec3], columns: u32, rows: u32) -> Vec<Link> {
    let mut links = Vec::new();
    let mut push = |a: u32, b: u32| {
        let (Some(pa), Some(pb)) = (positions.get(a as usize), positions.get(b as usize)) else {
            return;
        };
        links.push(Link {
            a,
            b,
            rest: (*pb - *pa).length(),
            kind: LinkKind::Bend,
        });
    };
    for row in 0..rows {
        for column in 0..columns.saturating_sub(2) {
            let here = row * columns + column;
            push(here, here + 2);
        }
    }
    for row in 0..rows.saturating_sub(2) {
        for column in 0..columns {
            let here = row * columns + column;
            push(here, here + 2 * columns);
        }
    }
    links
}

/// The largest sheet [`grid`] will build.
///
/// A cloth is a prop -- a curtain, a cape, a banner -- and every vertex of it is
/// integrated and constrained on the CPU every frame. 256 x 256 is already far
/// past what any of the three engines' cloth is used for, and it also keeps
/// `columns * rows` inside a `u32`, which an unbounded pair does not.
pub const MAX_CLOTH_VERTICES: u64 = 256 * 256;

/// Builds the flat sheet a [`Cloth`](bsengine_scene::Cloth) describes: a
/// `columns` by `rows` grid of vertices in the local XZ plane, `spacing` apart,
/// with vertex `row * columns + column` at `(column * spacing, 0, row *
/// spacing)`.
///
/// Returns nothing for a grid too small to have an edge -- one vertex across is
/// a row of loose points, not cloth.
///
/// The winding puts the surface normal along +Y, which
/// [`recompute_normals`] asserts by reproducing it. A sheet wound the other way
/// renders as its own back face: unlit, and lit scenes show it black.
pub fn grid(columns: u32, rows: u32, spacing: f32) -> Option<(Vec<Vertex>, Vec<u32>)> {
    if columns < 2
        || rows < 2
        || !spacing.is_finite()
        || spacing <= 0.0
        || columns as u64 * rows as u64 > MAX_CLOTH_VERTICES
    {
        return None;
    }
    let mut vertices = Vec::with_capacity((columns * rows) as usize);
    for row in 0..rows {
        for column in 0..columns {
            vertices.push(Vertex {
                position: [column as f32 * spacing, 0.0, row as f32 * spacing],
                color: [1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
                // Spanning 0..1 across the sheet, so one texture covers the
                // cloth whatever its resolution.
                uv: [
                    column as f32 / (columns - 1) as f32,
                    row as f32 / (rows - 1) as f32,
                ],
            });
        }
    }
    let mut indices = Vec::with_capacity(((columns - 1) * (rows - 1) * 6) as usize);
    for row in 0..rows - 1 {
        for column in 0..columns - 1 {
            let here = row * columns + column;
            let right = here + 1;
            let down = here + columns;
            let diagonal = down + 1;
            indices.extend_from_slice(&[here, down, right, right, down, diagonal]);
        }
    }
    Some((vertices, indices))
}

/// Replaces every vertex normal with the average of the faces meeting there,
/// weighted by face area (the cross product's length is twice the triangle's,
/// so using it unnormalized weights each face by its area for free).
///
/// A deforming mesh has to have this done every frame. Keeping the authored
/// normals instead lights a hanging curtain as though it were still lying flat,
/// which is a far more visible error than the geometry being slightly wrong.
pub fn recompute_normals(vertices: &mut [Vertex], indices: &[u32]) {
    for vertex in vertices.iter_mut() {
        vertex.normal = [0.0, 0.0, 0.0];
    }
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        if a >= vertices.len() || b >= vertices.len() || c >= vertices.len() {
            continue;
        }
        let (pa, pb, pc) = (
            Vec3::from(vertices[a].position),
            Vec3::from(vertices[b].position),
            Vec3::from(vertices[c].position),
        );
        let face = (pb - pa).cross(pc - pa);
        for index in [a, b, c] {
            let sum = Vec3::from(vertices[index].normal) + face;
            vertices[index].normal = sum.to_array();
        }
    }
    for vertex in vertices.iter_mut() {
        let normal = Vec3::from(vertex.normal);
        // A vertex whose faces cancelled out -- a fully creased fold -- has no
        // normal to normalize. +Y is the sheet's authored facing, and any
        // direction beats the NaN that normalizing zero would upload.
        vertex.normal = normal.try_normalize().unwrap_or(Vec3::Y).to_array();
    }
}

/// The tunables a step reads, mirroring the fields of the same name on
/// [`Cloth`](bsengine_scene::Cloth).
///
/// A struct rather than six more parameters: `step` stood at ten already, and
/// every one of these is read afresh each frame so that editing it on a live
/// cloth takes effect immediately -- baking any of them into the links at
/// generation time is what this shape exists to keep impossible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepParams {
    /// Constraint passes per step.
    pub iterations: u32,
    /// Enforces [`LinkKind::Structural`] links.
    pub stiffness: f32,
    /// Enforces [`LinkKind::Bend`] links. Separate because a fabric that barely
    /// stretches can still fold freely.
    pub bending_stiffness: f32,
    /// Fraction of velocity discarded each step.
    pub damping: f32,
    /// How close two of the sheet's own vertices may come before they push each
    /// other apart. Zero switches self-collision off.
    pub self_collision_distance: f32,
}

/// Advances the cloth one step, in place.
///
/// `previous` carries the velocity: a vertex's motion is the distance it
/// covered last step, which is what makes this Verlet integration and what
/// lets a constraint change a velocity just by moving a position. It is updated
/// in place alongside `positions`.
///
/// `pinned` holds indices that do not move. They still participate in every
/// constraint -- that is how the cloth hangs from them.
///
/// The tunables come in as [`StepParams`].
pub fn step(
    positions: &mut [Vec3],
    previous: &mut [Vec3],
    links: &[Link],
    pinned: &[u32],
    gravity: Vec3,
    dt: f32,
    params: &StepParams,
) {
    let StepParams {
        iterations,
        stiffness,
        bending_stiffness,
        damping,
        self_collision_distance,
    } = *params;
    if positions.len() != previous.len() || positions.is_empty() || dt <= 0.0 {
        return;
    }
    let stiffness = stiffness.clamp(0.0, 1.0);
    let bending_stiffness = bending_stiffness.clamp(0.0, 1.0);
    let damping = damping.clamp(0.0, 1.0);
    // A mask rather than a scan of `pinned`: this is asked twice per link per
    // iteration, so on a sheet of any size a linear search turns a list of held
    // corners into millions of comparisons a frame.
    let mut is_pinned = vec![false; positions.len()];
    for &index in pinned {
        if let Some(slot) = is_pinned.get_mut(index as usize) {
            *slot = true;
        }
    }
    let is_pinned = |i: usize| is_pinned[i];

    // Integrate. A pinned vertex is left exactly where it was, and its
    // `previous` is set to match so it carries no velocity into the next step --
    // otherwise unpinning one would fling it.
    for i in 0..positions.len() {
        if is_pinned(i) {
            previous[i] = positions[i];
            continue;
        }
        let velocity = (positions[i] - previous[i]) * (1.0 - damping);
        previous[i] = positions[i];
        positions[i] += velocity + gravity * dt * dt;
    }

    // Pull back toward the rest distances: each pass moves every link
    // `stiffness` of the way back. One pass is not enough because the links
    // fight each other -- correcting one pulls its neighbours off theirs -- so
    // the count is a setting, and raising it is what makes a cloth stiffer.
    for _ in 0..iterations {
        for link in links {
            let (a, b) = (link.a as usize, link.b as usize);
            if a >= positions.len() || b >= positions.len() {
                continue;
            }
            let delta = positions[b] - positions[a];
            let distance = delta.length();
            // Two vertices on top of each other have no direction to separate
            // along. Leaving them is stable; picking an arbitrary axis would
            // make the cloth jitter in a direction nothing asked for.
            if distance <= f32::EPSILON {
                continue;
            }
            let strength = match link.kind {
                LinkKind::Structural => stiffness,
                LinkKind::Bend => bending_stiffness,
            };
            // A bend link at zero stiffness is exactly the sheet before bending
            // existed, and skipping it here is what makes that free rather than
            // merely equivalent.
            if strength <= 0.0 {
                continue;
            }
            let correction = delta * (strength * (distance - link.rest) / distance);
            let (pa, pb) = (is_pinned(a), is_pinned(b));
            match (pa, pb) {
                // Both held: the link is whatever the pins make it. Moving
                // either would undo a pin, which is the one thing they promise.
                (true, true) => {}
                // One held: the free end does all of the moving, which is what
                // makes a pinned corner hold the whole sheet's weight.
                (true, false) => positions[b] -= correction,
                (false, true) => positions[a] += correction,
                (false, false) => {
                    positions[a] += correction * 0.5;
                    positions[b] -= correction * 0.5;
                }
            }
        }
    }

    // Self-collision after the constraint passes rather than inside them: it
    // builds a spatial index over every vertex, which is far too much to pay
    // once per iteration, and running it last is the stronger guarantee anyway
    // -- the positions that leave this function are the separated ones.
    push_apart_touching_vertices(positions, &is_pinned, self_collision_distance);
}

/// Pushes any two vertices closer than `distance` apart until they are exactly
/// that far, leaving pinned ones where they are.
///
/// This is what stops a cloth passing through itself -- a cape folding over its
/// own hem, a curtain gathered at the bottom. Unity spells it
/// `selfCollisionDistance` and Unreal `SelfCollisionThickness`; Godot has none.
///
/// # Why no exclusion list
///
/// The obvious worry is that this fights the links, since two vertices an edge
/// apart are always each other's nearest neighbours. It does not, as long as
/// `distance` is smaller than the shortest rest length -- Unity documents the
/// same rule for the same setting, and `step_params` in `cloth.rs` enforces it.
/// A linked pair only comes within `distance` when it is compressed below its
/// own rest length, and then the link wants them *further* apart than this
/// does. Both push the same way; this one simply asks for less.
///
/// # Why a spatial hash
///
/// Every pair is the honest formulation and is unusable: [`MAX_CLOTH_VERTICES`]
/// is 65536, so all-pairs would be two billion tests a frame. Bucketing by a
/// grid of `distance`-sized cells means each vertex only looks at the 27 cells
/// it could possibly touch -- and two vertices far apart in the mesh but folded
/// together in space, which is the whole case this exists for, land in the same
/// bucket regardless of their indices.
fn push_apart_touching_vertices(
    positions: &mut [Vec3],
    is_pinned: &dyn Fn(usize) -> bool,
    distance: f32,
) {
    if !distance.is_finite() || distance <= 0.0 {
        return;
    }
    let cell_of = |p: Vec3| {
        (
            (p.x / distance).floor() as i32,
            (p.y / distance).floor() as i32,
            (p.z / distance).floor() as i32,
        )
    };
    let mut buckets: std::collections::HashMap<(i32, i32, i32), Vec<usize>> =
        std::collections::HashMap::new();
    for (i, p) in positions.iter().enumerate() {
        buckets.entry(cell_of(*p)).or_default().push(i);
    }

    for i in 0..positions.len() {
        let (cx, cy, cz) = cell_of(positions[i]);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(cell) = buckets.get(&(cx + dx, cy + dy, cz + dz)) else {
                        continue;
                    };
                    for &j in cell {
                        // Each pair once, and never a vertex against itself.
                        if j <= i {
                            continue;
                        }
                        let delta = positions[j] - positions[i];
                        let apart = delta.length();
                        if apart >= distance || apart <= f32::EPSILON {
                            continue;
                        }
                        let correction = delta * ((distance - apart) / apart);
                        match (is_pinned(i), is_pinned(j)) {
                            (true, true) => {}
                            (true, false) => positions[j] += correction,
                            (false, true) => positions[i] -= correction,
                            (false, false) => {
                                positions[i] -= correction * 0.5;
                                positions[j] += correction * 0.5;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // What `Cloth::default()` uses, repeated here rather than exported: these
    // are this suite's parameters, and the component in `bsengine-scene` is the
    // one place the engine's actual defaults live.
    const DEFAULT_STIFFNESS: f32 = 0.9;
    /// What every test written before bending existed passed, and what
    /// `Cloth::default()` still uses.
    const NO_BEND: f32 = 0.0;
    /// What every test written before self-collision existed passed.
    const NO_SELF_COLLISION: f32 = 0.0;

    /// The settings this suite steps with, so a test names only what it varies.
    fn params() -> StepParams {
        StepParams {
            iterations: DEFAULT_ITERATIONS,
            stiffness: DEFAULT_STIFFNESS,
            bending_stiffness: NO_BEND,
            damping: DEFAULT_DAMPING,
            self_collision_distance: NO_SELF_COLLISION,
        }
    }
    const DEFAULT_DAMPING: f32 = 0.02;
    const DEFAULT_ITERATIONS: u32 = 8;

    /// A flat square sheet, `n` by `n` vertices one unit apart.
    ///
    /// ⚠️ Built by [`grid`], not by a copy of it here. A fixture that
    /// reimplements the geometry cannot observe a change to the real thing --
    /// a flipped winding or a transposed index layout would leave every test
    /// below green.
    fn sheet(n: u32) -> (Vec<Vec3>, Vec<u32>) {
        let (vertices, indices) = grid(n, n, 1.0).expect("a sheet at least 2x2");
        (
            vertices.iter().map(|v| Vec3::from(v.position)).collect(),
            indices,
        )
    }

    fn run(positions: &mut [Vec3], links: &[Link], pinned: &[u32], steps: usize) -> Vec<Vec3> {
        let mut previous = positions.to_vec();
        for _ in 0..steps {
            step(
                positions,
                &mut previous,
                links,
                pinned,
                Vec3::new(0.0, -9.81, 0.0),
                1.0 / 60.0,
                &params(),
            );
        }
        positions.to_vec()
    }

    #[test]
    fn the_grid_lays_vertices_out_where_pinning_indices_expect() {
        // ⚠️ Not square, and not unit-spaced: a transposed index layout or a
        // dropped `spacing` multiply is invisible on a 3x3 grid of unit steps,
        // and `pinned` is authored against exactly this arithmetic.
        let (vertices, _) = grid(5, 3, 0.25).expect("5x3");
        assert_eq!(vertices.len(), 15);
        for row in 0..3u32 {
            for column in 0..5u32 {
                let v = vertices[(row * 5 + column) as usize];
                assert_eq!(
                    v.position,
                    [column as f32 * 0.25, 0.0, row as f32 * 0.25],
                    "vertex at row {row}, column {column}"
                );
            }
        }
        // Corner UVs, so a texture covers the sheet rather than a corner of it.
        assert_eq!(vertices[0].uv, [0.0, 0.0]);
        assert_eq!(vertices[14].uv, [1.0, 1.0]);
    }

    #[test]
    fn the_grid_is_refused_when_it_has_no_edges() {
        assert!(grid(1, 4, 1.0).is_none(), "one column across");
        assert!(grid(4, 1, 1.0).is_none(), "one row deep");
        assert!(grid(4, 4, 0.0).is_none(), "no spacing");
        assert!(grid(4, 4, -1.0).is_none(), "negative spacing");
        assert!(grid(4, 4, f32::NAN).is_none(), "spacing is not a number");
        // ⚠️ And the pair must be multiplied as u64. A sheet this size is
        // refused, but `columns * rows` in u32 would panic on the way to
        // refusing it -- in a release build it would wrap to a small number and
        // allocate a sheet nobody asked for.
        assert!(grid(100_000, 100_000, 1.0).is_none(), "absurdly large");
        assert!(grid(257, 256, 1.0).is_none(), "one row past the cap");
        assert!(grid(256, 256, 1.0).is_some(), "exactly at the cap");
    }

    #[test]
    fn every_grid_edge_becomes_a_link_of_the_right_length() {
        let (vertices, indices) = grid(5, 3, 0.25).expect("5x3");
        let positions: Vec<Vec3> = vertices.iter().map(|v| Vec3::from(v.position)).collect();
        let links = links_from_indices(&positions, &indices);
        // 4 horizontal edges on each of 3 rows, 2 vertical on each of 5
        // columns, and one diagonal per cell.
        assert_eq!(links.len(), 4 * 3 + 5 * 2 + 4 * 2, "{}", links.len());
        let diagonal = 0.25 * 2.0_f32.sqrt();
        for link in &links {
            assert!(
                (link.rest - 0.25).abs() < 1.0e-6 || (link.rest - diagonal).abs() < 1.0e-6,
                "{link:?} is neither a side nor a diagonal"
            );
        }
    }

    #[test]
    fn a_flat_sheet_faces_up() {
        // ⚠️ This is the winding assertion. Wound the other way the cloth is
        // drawn from behind: unlit, and black in a lit scene. Nothing else
        // here would notice, because the solver does not care which way the
        // triangles run.
        let (mut vertices, indices) = grid(4, 3, 0.5).expect("4x3");
        recompute_normals(&mut vertices, &indices);
        for (i, v) in vertices.iter().enumerate() {
            let n = Vec3::from(v.normal);
            assert!(
                (n - Vec3::Y).length() < 1.0e-5,
                "vertex {i} should face up, got {n:?}"
            );
        }
    }

    #[test]
    fn normals_follow_the_surface_once_it_deforms() {
        let (mut vertices, indices) = grid(4, 4, 1.0).expect("4x4");
        // Tip the far row up, so the sheet becomes a ramp rising in +z. Its
        // normal has to lean back along -z.
        for vertex in &mut vertices[12..16] {
            vertex.position[1] = 1.0;
        }
        recompute_normals(&mut vertices, &indices);
        let tilted = Vec3::from(vertices[14].normal);
        assert!(
            (tilted.length() - 1.0).abs() < 1.0e-5,
            "normals must stay unit length, got {}",
            tilted.length()
        );
        assert!(
            tilted.z < -0.2,
            "a surface rising toward +z should face back along -z, got {tilted:?}"
        );
        assert!(tilted.y > 0.0, "and still upward, got {tilted:?}");
    }

    #[test]
    fn bend_links_span_two_along_each_axis() {
        // ⚠️ Not square, so a transposed loop is visible: 5 across and 3 down
        // gives 3 spans per row and 1 per column, not the same count twice.
        let (columns, rows) = (5u32, 3u32);
        let (vertices, _) = grid(columns, rows, 0.25).expect("5x3");
        let positions: Vec<Vec3> = vertices.iter().map(|v| Vec3::from(v.position)).collect();
        let bends = bend_links(&positions, columns, rows);
        // Every row has one span per vertex it can skip, and so does every
        // column -- stated as the formula rather than as a total, so the count
        // says which way round the two loops go.
        assert_eq!(
            bends.len() as u32,
            (columns - 2) * rows + columns * (rows - 2),
            "{bends:?}"
        );
        for link in &bends {
            assert_eq!(link.kind, LinkKind::Bend);
            assert!(
                (link.rest - 0.5).abs() < 1.0e-6,
                "a two-apart span rests at twice the spacing, got {}",
                link.rest
            );
            let step = link.b - link.a;
            assert!(
                step == 2 || step == 2 * columns,
                "spans must skip exactly one vertex along a row (2) or a column \
                 (2 * 5), got {step}"
            );
        }
    }

    #[test]
    fn a_sheet_too_narrow_to_fold_gets_no_bend_links_along_that_axis() {
        let (vertices, _) = grid(2, 5, 1.0).expect("2x5");
        let positions: Vec<Vec3> = vertices.iter().map(|v| Vec3::from(v.position)).collect();
        let bends = bend_links(&positions, 2, 5);
        // Two vertices across cannot hinge about a middle one, so every span
        // here runs down a column: 2 columns x (5 - 2) rows.
        assert_eq!(bends.len(), 2 * 3, "{bends:?}");
        assert!(bends.iter().all(|l| l.b - l.a == 4));
    }

    #[test]
    fn bending_makes_a_squashed_sheet_bow_instead_of_crumple() {
        // ⚠️ What a two-apart distance constraint actually does, arrived at by
        // measuring rather than by assuming. Two earlier fixtures asserted
        // things it cannot do:
        //
        // * A cantilever -- pinned along one edge, held out flat. A sheet
        //   swinging down about its pinned edge rotates *rigidly*, which leaves
        //   every distance in the mesh unchanged, two-apart spans included.
        //   Measured: drooped 3.9675 at bending 1.0 against 3.9676 at 0.
        // * Straightening a crease. The constraint resists curvature but has no
        //   sign, so it cannot tell a fold up from a fold down; asked to undo
        //   one it pushed the flap further over instead.
        //
        // What it does do is decide what happens to material with nowhere to
        // go. Pin the far row half a sheet's length from the near one and the
        // surface must give somewhere: without bending it concertinas flat in
        // its own plane, and with bending it bows out into a smooth arch.
        let measure = |bending: f32| {
            let (mut positions, indices) = sheet(5);
            let mut links = links_from_indices(&positions, &indices);
            let bends = bend_links(&positions, 5, 5);
            links.extend(bends.iter().copied());
            for column in 0..5u32 {
                let i = (4 * 5 + column) as usize;
                // A hair off the plane, so buckling has a direction to pick.
                // Exactly in-plane is a symmetric stalemate no solver leaves.
                positions[i] = Vec3::new(column as f32, 0.01, 2.0);
            }
            let pinned: Vec<u32> = (0..5).chain(20..25).collect();
            let mut previous = positions.clone();
            for _ in 0..600 {
                step(
                    &mut positions,
                    &mut previous,
                    &links,
                    &pinned,
                    Vec3::ZERO,
                    1.0 / 60.0,
                    &StepParams {
                        bending_stiffness: bending,
                        ..params()
                    },
                );
            }
            let span_error: f32 = bends
                .iter()
                .map(|l| {
                    ((positions[l.b as usize] - positions[l.a as usize]).length() - l.rest).abs()
                })
                .sum();
            let bow = positions.iter().map(|p| p.y.abs()).fold(0.0f32, f32::max);
            (span_error, bow)
        };

        let (limp_error, limp_bow) = measure(NO_BEND);
        let (stiff_error, stiff_bow) = measure(1.0);
        // Measured 9.95 / 0.30 and 0.01 / 1.76; the thresholds sit well clear
        // of both so this is about the behaviour, not about the numerics.
        assert!(
            limp_error > 5.0 && limp_bow < 0.1,
            "with no bending the sheet should crumple in its own plane: span              error {limp_error}, bow {limp_bow}"
        );
        assert!(
            stiff_error < 1.0 && stiff_bow > 1.0,
            "with bending it should bow out instead, holding its spans near              rest: span error {stiff_error}, bow {stiff_bow}"
        );
    }

    #[test]
    fn bending_at_zero_is_the_sheet_exactly_as_it_was() {
        // ⚠️ The compatibility claim, stated as an equality rather than a
        // tolerance. Every cloth authored before this field has
        // `bending_stiffness: 0`, and `generate_cloth` now hands the solver
        // bend links regardless -- so "the default changes nothing" is a
        // property of `step`, not of the component, and this is where it lives.
        let run = |with_bends: bool| {
            let (mut positions, indices) = sheet(4);
            let mut links = links_from_indices(&positions, &indices);
            if with_bends {
                links.extend(bend_links(&positions, 4, 4));
            }
            let pinned: Vec<u32> = (0..4).collect();
            let mut previous = positions.clone();
            for _ in 0..120 {
                step(
                    &mut positions,
                    &mut previous,
                    &links,
                    &pinned,
                    Vec3::new(0.0, -9.81, 0.0),
                    1.0 / 60.0,
                    &params(),
                );
            }
            positions
        };

        for (i, (with, without)) in run(true).iter().zip(run(false).iter()).enumerate() {
            assert_eq!(
                with, without,
                "vertex {i} moved because bend links were present at zero stiffness"
            );
        }
    }

    /// The closest any two vertices of `positions` come to each other.
    fn closest_pair(positions: &[Vec3]) -> f32 {
        let mut closest = f32::MAX;
        for i in 0..positions.len() {
            for j in i + 1..positions.len() {
                closest = closest.min((positions[j] - positions[i]).length());
            }
        }
        closest
    }

    #[test]
    fn self_collision_keeps_a_folded_sheet_out_of_itself() {
        // ⚠️ The fixture folds the sheet back over itself, which is the only
        // arrangement that can tell this feature from nothing: vertices an edge
        // apart are held by their links whatever this does, so the pair that
        // matters has to be one the links never touch -- here, row 0 laid
        // directly on top of row 4.
        let fold = |distance: f32| {
            let (mut positions, indices) = sheet(5);
            let links = links_from_indices(&positions, &indices);
            // Rows 3 and 4 folded back over rows 1 and 0, a hair above them.
            for row in 3..5u32 {
                for column in 0..5u32 {
                    let i = (row * 5 + column) as usize;
                    positions[i] = Vec3::new(column as f32, 0.02, (4 - row) as f32);
                }
            }
            let pinned: Vec<u32> = (0..5).collect();
            let mut previous = positions.clone();
            for _ in 0..120 {
                step(
                    &mut positions,
                    &mut previous,
                    &links,
                    &pinned,
                    Vec3::ZERO,
                    1.0 / 60.0,
                    &StepParams {
                        self_collision_distance: distance,
                        ..params()
                    },
                );
            }
            closest_pair(&positions)
        };

        let through = fold(NO_SELF_COLLISION);
        assert!(
            through < 0.1,
            "without self-collision the folded rows must stay lying on each \
             other -- the fixture is pointless otherwise: closest pair {through}"
        );
        let apart = fold(0.5);
        assert!(
            apart > 0.4,
            "with a 0.5 distance no two vertices may be closer than that: \
             closest pair {apart}"
        );
    }

    #[test]
    fn self_collision_sees_a_pair_that_straddles_two_buckets() {
        // ⚠️ The spatial hash's whole correctness condition, and the fold test
        // above cannot check it: those vertices happened to land in the same
        // bucket, so a version that only ever looked at its own cell passed.
        //
        // Two free vertices 0.02 apart, placed either side of a cell boundary
        // -- cells are `distance` wide, so 0.49 and 0.51 are in different ones.
        // Looking only at its own bucket, neither ever sees the other.
        let mut positions = vec![Vec3::new(0.49, 0.0, 0.0), Vec3::new(0.51, 0.0, 0.0)];
        let mut previous = positions.clone();
        step(
            &mut positions,
            &mut previous,
            &[],
            &[],
            Vec3::ZERO,
            1.0 / 60.0,
            &StepParams {
                self_collision_distance: 0.5,
                ..params()
            },
        );
        let apart = (positions[1] - positions[0]).length();
        assert!(
            (apart - 0.5).abs() < 1.0e-5,
            "the pair should have been pushed to exactly 0.5, got {apart}"
        );
    }

    #[test]
    fn self_collision_at_zero_is_the_sheet_exactly_as_it_was() {
        // The same equality `bending_at_zero_is_the_sheet_exactly_as_it_was`
        // states, for the same reason: every cloth authored before this field
        // has it at zero, and the guarantee belongs to `step` rather than to
        // the component.
        let run = |distance: f32| {
            let (mut positions, indices) = sheet(4);
            let links = links_from_indices(&positions, &indices);
            let pinned: Vec<u32> = (0..4).collect();
            let mut previous = positions.clone();
            for _ in 0..120 {
                step(
                    &mut positions,
                    &mut previous,
                    &links,
                    &pinned,
                    Vec3::new(0.0, -9.81, 0.0),
                    1.0 / 60.0,
                    &StepParams {
                        self_collision_distance: distance,
                        ..params()
                    },
                );
            }
            positions
        };
        for (i, (off, none)) in run(0.0).iter().zip(run(-1.0).iter()).enumerate() {
            assert_eq!(off, none, "vertex {i}");
        }
    }

    #[test]
    fn self_collision_never_moves_a_pinned_vertex() {
        // Pins are the one promise this component makes unconditionally, and a
        // pinned vertex is exactly the one another vertex is most likely to be
        // pressed against -- a cape's hem swinging back onto its own collar.
        let (mut positions, indices) = sheet(4);
        let links = links_from_indices(&positions, &indices);
        let pinned: Vec<u32> = (0..4).collect();
        // Fold the far row straight onto the pinned one.
        for column in 0..4u32 {
            let i = (3 * 4 + column) as usize;
            positions[i] = Vec3::new(column as f32, 0.01, 0.0);
        }
        let mut previous = positions.clone();
        for _ in 0..60 {
            step(
                &mut positions,
                &mut previous,
                &links,
                &pinned,
                Vec3::ZERO,
                1.0 / 60.0,
                &StepParams {
                    self_collision_distance: 0.5,
                    ..params()
                },
            );
        }
        for (i, p) in positions.iter().enumerate().take(4) {
            assert_eq!(
                *p,
                Vec3::new(i as f32, 0.0, 0.0),
                "pinned vertex {i} was pushed out of the way"
            );
        }
    }

    #[test]
    fn a_shared_edge_becomes_one_constraint() {
        // ⚠️ Two triangles sharing an edge must not give it two constraints:
        // enforced twice it is twice as stiff as the sheet's border, and the
        // cloth creases along its own triangulation.
        let (positions, indices) = sheet(2);
        let links = links_from_indices(&positions, &indices);
        // A 2x2 sheet has 4 vertices, 2 triangles, 5 distinct edges: four sides
        // and the shared diagonal.
        assert_eq!(links.len(), 5, "{links:?}");
        let mut pairs: Vec<(u32, u32)> = links.iter().map(|l| (l.a, l.b)).collect();
        pairs.sort_unstable();
        pairs.dedup();
        assert_eq!(pairs.len(), links.len(), "an edge appears twice: {links:?}");
    }

    #[test]
    fn rest_lengths_come_from_the_mesh_as_authored() {
        let (positions, indices) = sheet(2);
        let links = links_from_indices(&positions, &indices);
        for link in &links {
            let actual = (positions[link.b as usize] - positions[link.a as usize]).length();
            assert!(
                (link.rest - actual).abs() < 1.0e-6,
                "{link:?} against {actual}"
            );
        }
    }

    #[test]
    fn an_unpinned_cloth_falls() {
        let (mut positions, indices) = sheet(3);
        let links = links_from_indices(&positions, &indices);
        let before = positions[0].y;
        let after = run(&mut positions, &links, &[], 30);
        assert!(
            after[0].y < before - 0.1,
            "nothing is holding it: {} -> {}",
            before,
            after[0].y
        );
    }

    #[test]
    fn a_pinned_vertex_never_moves() {
        // The property every one of the three engines has, and the one a cloth
        // is unusable without: pin a corner and it stays on the character's
        // shoulder.
        let (mut positions, indices) = sheet(3);
        let links = links_from_indices(&positions, &indices);
        let pinned = [0u32, 2];
        let held: Vec<Vec3> = pinned.iter().map(|&i| positions[i as usize]).collect();
        let after = run(&mut positions, &links, &pinned, 120);
        for (slot, &i) in pinned.iter().enumerate() {
            assert!(
                (after[i as usize] - held[slot]).length() < 1.0e-5,
                "pinned vertex {i} drifted: {:?} from {:?}",
                after[i as usize],
                held[slot]
            );
        }
    }

    #[test]
    fn the_sheet_hangs_below_the_pins_it_hangs_from() {
        // Pinned along one edge, the rest of the sheet has to end up *under*
        // them. A solver that held the shape rigid would keep it flat, and the
        // pinned test above would pass just as well.
        let (mut positions, indices) = sheet(4);
        let links = links_from_indices(&positions, &indices);
        // The first row: z = 0.
        let pinned: Vec<u32> = (0..4).collect();
        let after = run(&mut positions, &links, &pinned, 180);

        let pin_y = after[0].y;
        // The far row, three rows down.
        for (i, p) in (12..16).zip(&after[12..16]) {
            assert!(
                p.y < pin_y - 0.5,
                "vertex {i} should hang below the pinned edge: {p:?} against a \
                 pin at y={pin_y}"
            );
        }
    }

    #[test]
    fn the_links_keep_their_lengths_while_it_hangs() {
        // ⚠️ What separates cloth from a rubber sheet. A solver that let the
        // links stretch would also "hang below the pins" -- it would simply
        // hang further every second.
        let (mut positions, indices) = sheet(4);
        let links = links_from_indices(&positions, &indices);
        let pinned: Vec<u32> = (0..4).collect();
        let after = run(&mut positions, &links, &pinned, 180);
        for link in &links {
            let length = (after[link.b as usize] - after[link.a as usize]).length();
            assert!(
                (length - link.rest).abs() < 0.25 * link.rest,
                "link {link:?} stretched to {length}"
            );
        }
    }

    /// Total distance every vertex covers over the second half of a run --
    /// how much the sheet is still moving once it has had time to settle.
    fn residual_motion(damping: f32, steps: usize) -> f32 {
        let (mut positions, indices) = sheet(4);
        let links = links_from_indices(&positions, &indices);
        let pinned: Vec<u32> = (0..4).collect();
        let mut previous = positions.clone();
        let mut total = 0.0;
        for s in 0..steps {
            let before = positions.clone();
            step(
                &mut positions,
                &mut previous,
                &links,
                &pinned,
                Vec3::new(0.0, -9.81, 0.0),
                1.0 / 60.0,
                &StepParams {
                    damping,
                    ..params()
                },
            );
            if s >= steps / 2 {
                total += positions
                    .iter()
                    .zip(&before)
                    .map(|(a, b)| (*a - *b).length())
                    .sum::<f32>();
            }
        }
        total
    }

    #[test]
    fn damping_settles_the_cloth_sooner() {
        // ⚠️ The obvious assertion here -- "it is moving less at the end than
        // at the start" -- passed with damping deleted entirely: projecting
        // constraints is itself dissipative, so the sheet slows down either
        // way. It certified a property the parameter had nothing to do with.
        //
        // What damping alone decides is *how much* motion is left, so that is
        // what this compares.
        let damped = residual_motion(0.2, 400);
        let undamped = residual_motion(0.0, 400);
        assert!(
            damped < undamped * 0.5,
            "damping should leave the sheet much quieter: damped {damped}, \
             undamped {undamped}"
        );
    }

    #[test]
    fn it_stays_finite_and_in_place_over_a_long_run() {
        // The failure a spring mesh is famous for: a correction that overshoots
        // feeds the next step, and the cloth leaves the world. Every assertion
        // above reads the sheet after a few seconds, by which point a diverging
        // solver is already NaN -- so the bound has to be stated.
        let (mut positions, indices) = sheet(4);
        let links = links_from_indices(&positions, &indices);
        let pinned: Vec<u32> = (0..4).collect();
        let mut previous = positions.clone();
        for _ in 0..900 {
            step(
                &mut positions,
                &mut previous,
                &links,
                &pinned,
                Vec3::new(0.0, -9.81, 0.0),
                1.0 / 60.0,
                &params(),
            );
        }
        // A 4x4 sheet of unit links hangs at most 3 units from its pins, so
        // nothing can legitimately be more than a few units from where it
        // started.
        for (i, p) in positions.iter().enumerate() {
            assert!(
                p.is_finite() && p.length() < 20.0,
                "vertex {i} left the world: {p:?}"
            );
        }
    }

    #[test]
    fn a_stiff_cloth_stretches_less_than_a_slack_one() {
        // The setting has to do something, and "less stretch" is what it means.
        let (positions, indices) = sheet(4);
        let links = links_from_indices(&positions, &indices);
        let pinned: Vec<u32> = (0..4).collect();

        let stretch = |stiffness: f32| {
            let mut p = positions.clone();
            let mut prev = p.clone();
            for _ in 0..120 {
                step(
                    &mut p,
                    &mut prev,
                    &links,
                    &pinned,
                    Vec3::new(0.0, -9.81, 0.0),
                    1.0 / 60.0,
                    &StepParams {
                        stiffness,
                        ..params()
                    },
                );
            }
            links
                .iter()
                .map(|l| ((p[l.b as usize] - p[l.a as usize]).length() - l.rest).abs())
                .sum::<f32>()
        };

        let stiff = stretch(1.0);
        let slack = stretch(0.1);
        assert!(
            stiff < slack,
            "stiffness should hold the links closer to rest: stiff {stiff}, \
             slack {slack}"
        );
    }

    #[test]
    fn a_degenerate_step_is_refused_rather_than_producing_nan() {
        let (mut positions, indices) = sheet(2);
        let links = links_from_indices(&positions, &indices);
        let mut previous = positions.clone();
        // A zero timestep divides nothing but integrates nothing either, and a
        // mismatched `previous` has no velocity to read.
        step(
            &mut positions,
            &mut previous,
            &links,
            &[],
            Vec3::new(0.0, -9.81, 0.0),
            0.0,
            &params(),
        );
        assert!(positions.iter().all(|p| p.is_finite()), "{positions:?}");
    }
}
