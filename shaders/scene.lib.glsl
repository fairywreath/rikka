#define SCENE_CONSTANTS_BINDING 0

layout(std140, set = MATERIAL_SET, binding = SCENE_CONSTANTS_BINDING) uniform SceneConstants
{
    mat4 view_projection;
    mat4 inverse_view_projection;
    mat4 world_to_camera;
    mat4 previous_view_projection;

    vec4 eye_position;

    vec4 light_position;
    float light_range;
    float light_intensity;

    uint dither_texture_index;

    float z_near;
    float z_far;

    float projection_00;
    float projection_11;

    uint frustum_cull_meshes;
    uint frustum_cull_meshlets;

    uint occlusion_cull_meshes;
    uint occlusion_cull_meshlets;

    vec2 resolution;

    float aspect_ratio;

    vec4 frustum_planes[6];

    // 32 bytes padding
    uint _pad0;
    uint _pad1;
    uint _pad2;
    uint _pad3;
};

float linearize_depth(float depth)
{
    // Map to [0, 1] depth
    return z_near * z_far / (z_far + depth * (z_near - z_far));
}

float dither(vec2 screen_pixel_position, float value)
{
    float dither_value = texelFetch(global_textures[nonuniformEXT(dither_texture_index)],
                                    ivec2(int(screen_pixel_position.x) % 4, int(screen_pixel_position.y) % 4), 0)
                             .r;
    return value - dither_value;
}
