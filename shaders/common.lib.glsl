#extension GL_ARB_shader_draw_parameters : enable
#extension GL_EXT_nonuniform_qualifier : enable

#define MATERIAL_SET 0
#define BINDLESS_SET 1

#define BINDLESS_TEXTURES_BINDING 15
#define BINDLESS_WRITE_ONLY_IMAGE_BINDING 16

layout(set = BINDLESS_SET, binding = BINDLESS_TEXTURES_BINDING) uniform sampler2D global_textures[];
layout(set = BINDLESS_SET, binding = BINDLESS_TEXTURES_BINDING) uniform sampler3D global_textures_3d[];
layout(set = BINDLESS_SET,
       binding = BINDLESS_WRITE_ONLY_IMAGE_BINDING) writeonly uniform image2D global_write_only_images[];

// u32::MAX
#define INVALID_TEXTURE_INDEX 4294967295

#define PI 3.1415926538

vec3 decode_srgb(vec3 c)
{
    vec3 result;

    if (c.r <= 0.04045)
    {
        result.r = c.r / 12.92;
    }
    else
    {
        result.r = pow((c.r + 0.055) / 1.055, 2.4);
    }

    if (c.g <= 0.04045)
    {
        result.g = c.g / 12.92;
    }
    else
    {
        result.g = pow((c.g + 0.055) / 1.055, 2.4);
    }

    if (c.b <= 0.04045)
    {
        result.b = c.b / 12.92;
    }
    else
    {
        result.b = pow((c.b + 0.055) / 1.055, 2.4);
    }

    return clamp(result, 0.0, 1.0);
}

vec3 encode_srgb(vec3 c)
{
    vec3 result;

    if (c.r <= 0.0031308)
    {
        result.r = c.r * 12.92;
    }
    else
    {
        result.r = 1.055 * pow(c.r, 1.0 / 2.4) - 0.055;
    }

    if (c.g <= 0.0031308)
    {
        result.g = c.g * 12.92;
    }
    else
    {
        result.g = 1.055 * pow(c.g, 1.0 / 2.4) - 0.055;
    }

    if (c.b <= 0.0031308)
    {
        result.b = c.b * 12.92;
    }
    else
    {
        result.b = 1.055 * pow(c.b, 1.0 / 2.4) - 0.055;
    }

    return clamp(result, 0.0, 1.0);
}

float heaviside(float v)
{
    float result = 0.0;
    if (v > 0.0)
    {
        result = 1.0;
    }

    return result;
}

vec2 sign_not_zero(vec2 v)
{
    return vec2((v.x >= 0.0) ? 1.0 : -1.0, (v.y >= 0.0) ? 1.0 : -1.0);
}

vec3 octahedral_decode(vec2 f)
{
    vec3 n = vec3(f.x, f.y, 1.0 - abs(f.x) - abs(f.y));
    float t = max(-n.z, 0.0);
    n.x += n.x >= 0.0 ? -t : t;
    n.y += n.y >= 0.0 ? -t : t;

    return normalize(n);
}

vec2 octahedral_encode(vec3 n)
{
    // Project the sphere onto the octahedron, and then onto the xy plane
    vec2 p = n.xy * (1.0f / (abs(n.x) + abs(n.y) + abs(n.z)));
    // Reflect the folds of the lower hemisphere over the diagonals
    return (n.z < 0.0f) ? ((1.0 - abs(p.yx)) * sign_not_zero(p)) : p;
}

// Get world position from raw depth
vec3 world_position_from_depth(vec2 uv, float raw_depth, mat4 inverse_view_projection)
{
    // Homogenous vector scaled to [-1, 1] with `z` component containing depth value
    vec4 H = vec4(uv.x * 2 - 1, uv.y * -2 + 1, raw_depth, 1);

    // Transform homogenous vector from view space to world space
    vec4 D = inverse_view_projection * H;

    // Perspective division: obtain cartesian(non-homogenous) coordinates
    return D.xyz / D.w;
}

vec2 uv_from_pixels(ivec2 pixel_position, uint width, uint height)
{
    return pixel_position / vec2((width - 1) * 1.f, (height - 1) * 1.f);
}
