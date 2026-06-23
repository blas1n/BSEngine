pub trait RHI: Send + Sync {
    fn create_mesh(&self) -> Box<dyn RHIMesh>;
    fn create_shader(&self, src: &str) -> Box<dyn RHIShader>;
    fn create_texture(&self) -> Box<dyn RHITexture>;
}

pub trait RHIMesh: Send + Sync {}
pub trait RHIShader: Send + Sync {}
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
