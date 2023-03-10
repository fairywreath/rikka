pub mod camera;
pub mod gltf;
pub mod renderer;

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
