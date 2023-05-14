#version 460 core

#pragma RIKKA_REQUIRE(common.lib.glsl)
#pragma RIKKA_REQUIRE(mesh.lib.glsl)
#pragma RIKKA_REQUIRE(scene.lib.glsl)

layout(location = 0) in vec2 vTexcoord0;
layout(location = 1) in vec4 vNormal_BiTanX;
layout(location = 2) in vec4 vTangent_BiTanY;
layout(location = 3) in vec4 vPosition_BiTanZ;
layout(location = 4) in flat uint mesh_draw_index;

#if DEBUG
layout(location = 5) in vec4 vColour;
#endif

layout(location = 0) out vec4 color_out;
layout(location = 1) out vec2 normal_out;
layout(location = 2) out vec4 occlusion_roughness_metalness_out;
layout(location = 3) out vec4 emissive_out;

void main()
{
    MeshMaterial mesh_draw = mesh_materials[mesh_draw_index];
    uint flags = mesh_draw.draw_flags;

    vec3 world_position = vPosition_BiTanZ.xyz;
    vec3 normal = normalize(vNormal_BiTanX.xyz);
    if ((flags & DrawFlags_HasNormals) == 0)
    {
        normal = normalize(cross(dFdx(world_position), dFdy(world_position)));
    }

    vec3 tangent = normalize(vTangent_BiTanY.xyz);
    vec3 bitangent = normalize(vec3(vNormal_BiTanX.w, vTangent_BiTanY.w, vPosition_BiTanZ.w));
    if ((flags & DrawFlags_HasTangents) == 0)
    {
        vec3 uv_dx = dFdx(vec3(vTexcoord0, 0.0));
        vec3 uv_dy = dFdy(vec3(vTexcoord0, 0.0));

        // https://github.com/KhronosGroup/glTF-Sample-Viewer
        vec3 t_ =
            (uv_dy.t * dFdx(world_position) - uv_dx.t * dFdy(world_position)) / (uv_dx.s * uv_dy.t - uv_dy.s * uv_dx.t);
        tangent = normalize(t_ - normal * dot(normal, t_));

        bitangent = cross(normal, tangent);
    }

    vec4 base_colour = mesh_draw.base_color_factor;

    if (mesh_draw.diffuse_texture_index != INVALID_TEXTURE_INDEX)
    {
        vec3 texture_colour =
            decode_srgb(texture(global_textures[nonuniformEXT(mesh_draw.diffuse_texture_index)], vTexcoord0).rgb);
        base_colour *= vec4(texture_colour, 1.0);
    }

    bool useAlphaMask = (flags & DrawFlags_AlphaMask) != 0;
    if (useAlphaMask && base_colour.a < mesh_draw.alpha_cutoff)
    {
        discard;
    }

    bool use_alpha_dither = (flags & DrawFlags_AlphaDither) != 0;
    if (use_alpha_dither)
    {
        float dithered_alpha = dither(gl_FragCoord.xy, base_colour.a);
        if (dithered_alpha < 0.001f)
        {
            discard;
        }
    }

    if (gl_FrontFacing == false)
    {
        tangent *= -1.0;
        bitangent *= -1.0;
        normal *= -1.0;
    }

    if (mesh_draw.normal_texture_index != INVALID_TEXTURE_INDEX)
    {
        // Map normals from [0, 1] to [-1, 1]
        vec3 bump_normal = normalize(
            texture(global_textures[nonuniformEXT(mesh_draw.normal_texture_index)], vTexcoord0).rgb * 2.0 - 1.0);
        mat3 TBN = mat3(tangent, bitangent, normal);

        normal = normalize(TBN * normalize(bump_normal));
    }

    normal_out.rg = octahedral_encode(normal);

    float metalness = 0.0;
    float roughness = 0.0;
    float occlusion = 0.0;

    roughness = mesh_draw.metallic_roughness_occlusion_factor.x;
    metalness = mesh_draw.metallic_roughness_occlusion_factor.y;

    if (mesh_draw.metallic_roughness_texture_index != INVALID_TEXTURE_INDEX)
    {
        vec4 rm = texture(global_textures[nonuniformEXT(mesh_draw.metallic_roughness_texture_index)], vTexcoord0);

        // Green channel contains roughness values
        roughness *= rm.g;

        // Blue channel contains metalness
        metalness *= rm.b;
    }

    occlusion = mesh_draw.metallic_roughness_occlusion_factor.z;
    if (mesh_draw.occlusion_texture_index != INVALID_TEXTURE_INDEX)
    {
        vec4 o = texture(global_textures[nonuniformEXT(mesh_draw.occlusion_texture_index)], vTexcoord0);
        // Red channel for occlusion value
        occlusion *= o.r;
    }

    emissive_out = vec4(mesh_draw.emissive.rgb, 1.0);
    uint emissive_texture = uint(mesh_draw.emissive.w);
    if (emissive_texture != INVALID_TEXTURE_INDEX)
    {
        emissive_out *=
            vec4(decode_srgb(texture(global_textures[nonuniformEXT(emissive_texture)], vTexcoord0).rgb), 1.0);
    }

    occlusion_roughness_metalness_out.rgb = vec3(occlusion, roughness, metalness);

#if DEBUG
    color_out = vColour;
#else
    color_out = base_colour;
#endif
}
