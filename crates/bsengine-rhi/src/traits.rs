/// Entry point a concrete GPU backend implements to create RHI resources.
pub trait RHI: Send + Sync {
    /// Creates a new, backend-specific mesh resource.
    fn create_mesh(&self) -> Box<dyn RHIMesh>;
    /// Compiles `src` into a backend-specific shader resource.
    fn create_shader(&self, src: &str) -> Box<dyn RHIShader>;
    /// Creates a new, backend-specific texture resource.
    fn create_texture(&self) -> Box<dyn RHITexture>;
}

/// Opaque handle to a GPU mesh resource owned by a backend implementation.
pub trait RHIMesh: Send + Sync {}
/// Opaque handle to a compiled GPU shader resource owned by a backend implementation.
pub trait RHIShader: Send + Sync {}
/// Opaque handle to a GPU texture resource owned by a backend implementation.
pub trait RHITexture: Send + Sync {}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRHI;
    impl RHI for MockRHI {
        fn create_mesh(&self) -> Box<dyn RHIMesh> {
            Box::new(MockMesh)
        }
        fn create_shader(&self, _src: &str) -> Box<dyn RHIShader> {
            Box::new(MockShader)
        }
        fn create_texture(&self) -> Box<dyn RHITexture> {
            Box::new(MockTexture)
        }
    }

    struct MockMesh;
    impl RHIMesh for MockMesh {}

    struct MockShader;
    impl RHIShader for MockShader {}

    struct MockTexture;
    impl RHITexture for MockTexture {}

    #[test]
    fn mock_rhi_implements_trait() {
        let rhi: Box<dyn RHI> = Box::new(MockRHI);
        let _mesh = rhi.create_mesh();
        let _shader = rhi.create_shader("void main() {}");
        let _texture = rhi.create_texture();
    }
}
