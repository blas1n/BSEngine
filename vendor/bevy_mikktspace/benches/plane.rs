use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

/// A unit plane with corners at `(0, 0)` and `(1, 1)` in the XY plane.
/// `N` is the number of quads used to subdivide this plane.
struct SubdividedPlane<const N: usize>;

impl<const N: usize> bevy_mikktspace::Geometry for SubdividedPlane<N> {
    fn num_faces(&self) -> usize {
        N * N
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        4
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        let f_y = face % N;
        let f_x = face / N;

        let (v_x, v_y) = match vert {
            0 => (f_x, f_y),
            1 => (f_x, f_y + 1),
            2 => (f_x + 1, f_y + 1),
            3 => (f_x + 1, f_y),
            _ => panic!(),
        };

        [
            v_x as f32 / (N as f32 + 1.),
            v_y as f32 / (N as f32 + 1.),
            0.,
        ]
    }

    fn normal(&self, _face: usize, _vert: usize) -> [f32; 3] {
        [0., 0., 1.]
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        let f_y = face % N;
        let f_x = face / N;

        let (v_x, v_y) = match vert {
            0 => (f_x, f_y),
            1 => (f_x, f_y + 1),
            2 => (f_x + 1, f_y + 1),
            3 => (f_x + 1, f_y),
            _ => panic!(),
        };

        [v_x as f32 / (N as f32 + 1.), v_y as f32 / (N as f32 + 1.)]
    }

    fn set_tangent(
        &mut self,
        tangent_space: Option<bevy_mikktspace::TangentSpace>,
        _face: usize,
        _vert: usize,
    ) {
        let _ = black_box(tangent_space);
    }
}

impl<const N: usize> mikktspace_sys::MikkTSpaceInterface for SubdividedPlane<N> {
    fn get_num_faces(&self) -> usize {
        <Self as bevy_mikktspace::Geometry>::num_faces(&self)
    }

    fn get_num_vertices_of_face(&self, face: usize) -> usize {
        <Self as bevy_mikktspace::Geometry>::num_vertices_of_face(&self, face)
    }

    fn get_position(&self, face: usize, vert: usize) -> [f32; 3] {
        <Self as bevy_mikktspace::Geometry>::position(&self, face, vert)
    }

    fn get_normal(&self, face: usize, vert: usize) -> [f32; 3] {
        <Self as bevy_mikktspace::Geometry>::normal(&self, face, vert)
    }

    fn get_tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        <Self as bevy_mikktspace::Geometry>::tex_coord(&self, face, vert)
    }

    fn set_tspace_basic(&mut self, tangent: [f32; 3], _sign: f32, _face: usize, _vert: usize) {
        let _ = black_box(tangent);
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    // Bevy's implementation
    c.bench_function("bevy", |b| {
        b.iter(|| {
            let _ = black_box(bevy_mikktspace::generate_tangents(black_box(
                &mut SubdividedPlane::<32>,
            )));
        })
    });

    // Original C implementation over FFI
    c.bench_function("original", |b| {
        b.iter(|| {
            let _ = black_box(mikktspace_sys::gen_tang_space_default(black_box(
                &mut SubdividedPlane::<32>,
            )));
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
