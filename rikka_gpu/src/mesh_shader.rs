use rikka_core::ash::extensions::nv::MeshShader;

use crate::factory::DeviceGuard;

pub struct MeshShaderContext {
    pub functions: MeshShader,
    pub device: DeviceGuard,
}

impl MeshShaderContext {
    pub fn new(device: DeviceGuard) -> Self {
        Self {
            functions: MeshShader::new(device.instance().raw(), device.raw()),
            device,
        }
    }
}
