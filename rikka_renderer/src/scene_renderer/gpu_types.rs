use rikka_core::{
    nalgebra::{Matrix4, Vector2, Vector3, Vector4},
    vk,
};

#[derive(Copy, Clone)]
#[repr(C, align(16))]
pub struct GpuMeshMaterial {
    pub diffuse_texture_index: u32,
    pub metallic_roughness_texture_index: u32,
    pub normal_texture_index: u32,
    pub occlusion_texture_index: u32,

    /// x, y, z - emissive factor, w - emissive texture index
    pub emissive: Vector4<f32>,

    pub base_color_factor: Vector4<f32>,
    pub metallic_roughness_occlusion_factor: Vector4<f32>,

    pub draw_flags: u32,
    pub alpha_cutoff: f32,

    pub vertex_offset: u32,
    pub mesh_index: u32,

    pub meshlet_offset: u32,
    pub meshlet_count: u32,

    _pad0: u32,
    _pad1: u32,
}

#[derive(Copy, Clone)]
#[repr(C, align(16))]
pub struct GpuMeshInstanceData {
    pub model: Matrix4<f32>,
    pub inverse_model: Matrix4<f32>,

    pub mesh_index: u32,

    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[derive(Copy, Clone)]
#[repr(C, align(16))]
pub struct GpuMeshDrawCommand {
    pub draw_id: u32,
    pub indirect_indexed: vk::DrawIndexedIndirectCommand,
    pub mesh_tasks_indirect: vk::DrawMeshTasksIndirectCommandNV,
}

#[derive(Copy, Clone)]
#[repr(C, align(16))]
pub struct GpuMeshDrawCounts {
    pub opaque_mesh_visible_count: u32,
    pub opaque_mesh_culled_count: u32,
    pub transparent_mesh_visible_count: u32,
    pub transparent_mesh_culled_count: u32,

    pub total_count: u32,
    pub depth_pyramid_texture_index: u32,
    pub is_late: u32,

    _pad0: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct GpuMeshletVertexData {
    pub normal: Vector4<u8>,
    pub tangent: Vector4<u8>,
    pub tex_coords: Vector2<u16>,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct GpuMeshletVertexPosition {
    pub position: Vector3<f32>,

    _pad0: u32,
}

#[derive(Copy, Clone)]
#[repr(C, align(16))]
pub struct GpuMeshlet {
    pub center: Vector3<f32>,
    pub radius: f32,

    pub cone_axis: Vector3<i8>,
    pub cone_cutoff: i8,

    pub data_offset: u32,
    pub mesh_index: u32,

    pub vertex_count: u8,
    pub triangle_count: u8,
}

/// Per-frame scene constants
#[derive(Clone, Copy)]
#[repr(C)]
pub struct GpuSceneConstants {
    pub view_projection: Matrix4<f32>,
    pub inverse_view_projection: Matrix4<f32>,
    pub previous_view_projection: Matrix4<f32>,
    pub world_to_camera: Matrix4<f32>,

    pub eye_position: Vector4<f32>,

    pub light_position: Vector4<f32>,
    pub light_range: f32,
    pub light_intensity: f32,

    pub dither_texture_index: u32,

    pub z_near: f32,
    pub z_far: f32,

    pub projection_00: f32,
    pub projection_11: f32,

    pub frustum_cull_meshes: u32,
    pub frustum_cull_meshlets: u32,

    pub occlusion_cull_mesh: u32,
    pub occlusion_cull_meshlets: u32,

    pub resolution_x: f32,
    pub resolution_y: f32,
    pub aspect_ratio: f32,

    pub frustum_planes: [Vector4<f32>; 6],

    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}
