use nalgebra::Vector4;

pub mod camera;
pub mod gltf;
pub mod renderer;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct MaterialData {
    pub base_color_factor: Vector4<f32>,
    pub diffuse_texture: u32,
    // Occulsion metallic roughness
    pub omr_texture: u32,
    pub normal_texture: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    pub positions: [f32; 3],
    // pub tex_coords: [f32; 2],
}

pub fn cube_vertices() -> [Vertex; 36] {
    [
        Vertex {
            positions: [-1.0, -1.0, -1.0],
        }, // triangle 1 : begin
        Vertex {
            positions: [-1.0, -1.0, 1.0],
        },
        Vertex {
            positions: [-1.0, 1.0, 1.0],
        }, // triangle 1 : end
        Vertex {
            positions: [1.0, 1.0, -1.0],
        }, // triangle 2 : begin
        Vertex {
            positions: [-1.0, -1.0, -1.0],
        },
        Vertex {
            positions: [-1.0, 1.0, -1.0],
        }, // triangle 2 : end
        Vertex {
            positions: [1.0, -1.0, 1.0],
        },
        Vertex {
            positions: [-1.0, -1.0, -1.0],
        },
        Vertex {
            positions: [1.0, -1.0, -1.0],
        },
        Vertex {
            positions: [1.0, 1.0, -1.0],
        },
        Vertex {
            positions: [1.0, -1.0, -1.0],
        },
        Vertex {
            positions: [-1.0, -1.0, -1.0],
        },
        Vertex {
            positions: [-1.0, -1.0, -1.0],
        },
        Vertex {
            positions: [-1.0, 1.0, 1.0],
        },
        Vertex {
            positions: [-1.0, 1.0, -1.0],
        },
        Vertex {
            positions: [1.0, -1.0, 1.0],
        },
        Vertex {
            positions: [-1.0, -1.0, 1.0],
        },
        Vertex {
            positions: [-1.0, -1.0, -1.0],
        },
        Vertex {
            positions: [-1.0, 1.0, 1.0],
        },
        Vertex {
            positions: [-1.0, -1.0, 1.0],
        },
        Vertex {
            positions: [1.0, -1.0, 1.0],
        },
        Vertex {
            positions: [1.0, 1.0, 1.0],
        },
        Vertex {
            positions: [1.0, -1.0, -1.0],
        },
        Vertex {
            positions: [1.0, 1.0, -1.0],
        },
        Vertex {
            positions: [1.0, -1.0, -1.0],
        },
        Vertex {
            positions: [1.0, 1.0, 1.0],
        },
        Vertex {
            positions: [1.0, -1.0, 1.0],
        },
        Vertex {
            positions: [1.0, 1.0, 1.0],
        },
        Vertex {
            positions: [1.0, 1.0, -1.0],
        },
        Vertex {
            positions: [-1.0, 1.0, -1.0],
        },
        Vertex {
            positions: [1.0, 1.0, 1.0],
        },
        Vertex {
            positions: [-1.0, 1.0, -1.0],
        },
        Vertex {
            positions: [-1.0, 1.0, 1.0],
        },
        Vertex {
            positions: [1.0, 1.0, 1.0],
        },
        Vertex {
            positions: [-1.0, 1.0, 1.0],
        },
        Vertex {
            positions: [1.0, -1.0, 1.0],
        },
    ]
}
