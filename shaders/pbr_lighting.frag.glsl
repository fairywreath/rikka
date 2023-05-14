#version 460 core

#pragma shader_stage(fragment)

#pragma RIKKA_REQUIRE(common.lib.glsl)
#pragma RIKKA_REQUIRE(scene.lib.glsl)

layout(std140, set = 0, binding = 1) uniform LightingConstants
{
    // x = albedo index, y = roughness index, z = normal index, w = position index.
    // Occlusion and roughness are encoded in the same texture
    uvec4 textures;

    uint output_index; // Used by compute
    uint output_width;
    uint output_height;
    uint emissive_index;
};

layout(location = 0) in vec2 vTexcoord0;

layout(location = 0) out vec4 out_frag_color;

vec4 calculate_lighting(vec4 base_colour, vec3 orm, vec3 normal, vec3 emissive, vec3 vPosition)
{
    vec3 V = normalize(eye_position.xyz - vPosition);
    vec3 L = normalize(light_position.xyz - vPosition);
    vec3 N = normal;
    vec3 H = normalize(L + V);

    float occlusion = orm.r;
    float roughness = orm.g;
    float metalness = orm.b;

    float alpha = pow(roughness, 2.0);

    // https://www.khronos.org/registry/glTF/specs/2.0/glTF-2.0.html#specular-brdf
    float NdotH = clamp(dot(N, H), 0, 1);
    float alpha_squared = alpha * alpha;
    float d_denom = (NdotH * NdotH) * (alpha_squared - 1.0) + 1.0;
    float distribution = (alpha_squared * heaviside(NdotH)) / (PI * d_denom * d_denom);

    float NdotL = clamp(dot(N, L), 0, 1);
    float NdotV = clamp(dot(N, V), 0, 1);
    float HdotL = clamp(dot(H, L), 0, 1);
    float HdotV = clamp(dot(H, V), 0, 1);

    float distance = length(light_position.xyz - vPosition);
    float intensity = light_intensity * max(min(1.0 - pow(distance / light_range, 4.0), 1.0), 0.0) / pow(distance, 2.0);

    vec3 material_colour = vec3(0, 0, 0);
    if (NdotL > 0.0 || NdotV > 0.0)
    {
        float visibility =
            (heaviside(HdotL) / (abs(NdotL) + sqrt(alpha_squared + (1.0 - alpha_squared) * (NdotL * NdotL)))) *
            (heaviside(HdotV) / (abs(NdotV) + sqrt(alpha_squared + (1.0 - alpha_squared) * (NdotV * NdotV))));

        float specular_brdf = intensity * NdotL * visibility * distribution;

        vec3 diffuse_brdf = intensity * NdotL * (1 / PI) * base_colour.rgb;

        // NOTE(marco): f0 in the formula notation refers to the base colour here
        vec3 conductor_fresnel = specular_brdf * (base_colour.rgb + (1.0 - base_colour.rgb) * pow(1.0 - abs(HdotV), 5));

        // NOTE(marco): f0 in the formula notation refers to the value derived from ior = 1.5
        float f0 = 0.04; // pow( ( 1 - ior ) / ( 1 + ior ), 2 )
        float fr = f0 + (1 - f0) * pow(1 - abs(HdotV), 5);
        vec3 fresnel_mix = mix(diffuse_brdf, vec3(specular_brdf), fr);

        material_colour = mix(fresnel_mix, conductor_fresnel, metalness);
    }

    material_colour += emissive;

    return vec4(encode_srgb(material_colour), base_colour.a);
}

void main()
{
    vec4 base_colour = texture(global_textures[nonuniformEXT(textures.x)], vTexcoord0);
    vec3 orm = texture(global_textures[nonuniformEXT(textures.y)], vTexcoord0).rgb;
    vec2 encoded_normal = texture(global_textures[nonuniformEXT(textures.z)], vTexcoord0).rg;
    vec3 normal = octahedral_decode(encoded_normal);
    vec3 vPosition = texture(global_textures[nonuniformEXT(textures.w)], vTexcoord0).rgb;
    vec3 emissive = texture(global_textures[nonuniformEXT(emissive_index)], vTexcoord0).rgb;

    out_frag_color = calculate_lighting(base_colour, orm, normal, emissive, vPosition);
}
