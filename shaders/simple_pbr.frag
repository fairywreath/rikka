#version 460 core

layout(location = 0) in vec3 in_Position;
layout(location = 1) in vec2 in_TexCoord0;
layout(location = 2) in vec3 in_Normal;
layout(location = 3) in vec4 in_Tangent;

layout(location = 0) out vec4 out_FragColor;

layout(std140, binding = 0) uniform UniformData
{
    mat4 model;
    mat4 view;
    mat4 proj;

    vec4 eye;
    vec4 light;
};

layout(std140, binding = 4) uniform MaterialUniform
{
    vec4 baseColorFactor;

    // Indices to the global bindless texture array
    uint diffuseTextureIndex;
    uint occlusionRoughnessMetalnessTextureIndex;
    uint normalTextureIndex;
};

#extension GL_EXT_nonuniform_qualifier : enable

layout(set = 1, binding = 10) uniform sampler2D globalTextures[];
// layout(set = 1, binding = 10) uniform sampler3D globalTextures3D[];

#define PI 3.1415926535897932384626433832795

#define INVALID_TEXTURE_INDEX 4294967295

float decode_srgb_component(float value)
{
    float result = value / 12.92;
    if (value > 0.04045)
    {
        result = pow((value + 0.055) / 1.055, 2.4);
    }
    return result;
}

// Srgb gamma correction
vec3 decode_srgb(vec3 color)
{
    vec3 result = vec3(decode_srgb_component(color.r), decode_srgb_component(color.g), decode_srgb_component(color.b));

    return clamp(result, 0.0, 1.0);
}

float encode_srgb_component(float value)
{
    float result = value * 12.92;
    if (value > 0.0031308)
    {
        result = 1.055 * pow(value, 1.0 / 2.4) - 0.055;
    }
    return result;
}

vec3 encode_srgb(vec3 color)
{

    vec3 result = vec3(encode_srgb_component(color.r), encode_srgb_component(color.g), encode_srgb_component(color.b));
    return clamp(result, 0.0, 1.0);
}

float heaviside(float v)
{
    if (v > 0.0)
        return 1.0;
    return 0.0;
}

void main()
{
    vec4 diffuseTexture = texture(globalTextures[diffuseTextureIndex], in_TexCoord0);
    vec4 omr = texture(globalTextures[occlusionRoughnessMetalnessTextureIndex], in_TexCoord0);
    vec4 normalTexture = texture(globalTextures[normalTextureIndex], in_TexCoord0);

    vec3 tangent = normalize(in_Tangent.xyz);
    vec3 bitangent = cross(normalize(in_Normal), tangent) * in_Tangent.w;

    // Map normals from [0, 1] to [-1, 1]
    vec3 bump_normal = normalize(normalTexture.rgb * 2.0 - 1.0);

    mat3 TBN = transpose(mat3(tangent, bitangent, normalize(in_Normal)));

    vec3 V = normalize(TBN * (eye.xyz - in_Position.xyz));
    vec3 L = normalize(TBN * (light.xyz - in_Position.xyz));
    vec3 N = bump_normal;
    vec3 H = normalize(L + V);

    // Green channel - roughness
    float roughness = omr.g;
    float alpha = pow(roughness, 2.0);

    // Blue channel - metalness
    float metalness = omr.b;

    vec4 base_color = diffuseTexture * baseColorFactor;
    base_color.rgb = decode_srgb(base_color.rgb);

    // Specular brdf
    float NdotH = dot(N, H);
    float alpha_squared = alpha * alpha;
    float d_denom = (NdotH * NdotH) * (alpha_squared - 1.0) + 1.0;
    float distribution = (alpha_squared * heaviside(NdotH)) / (PI * d_denom * d_denom);

    float NdotL = dot(N, L);
    float NdotV = dot(N, V);
    float HdotL = dot(H, L);
    float HdotV = dot(H, V);

    float visibility =
        (heaviside(HdotL) / (abs(NdotL) + sqrt(alpha_squared + (1.0 - alpha_squared) * (NdotL * NdotL)))) *
        (heaviside(HdotV) / (abs(NdotV) + sqrt(alpha_squared + (1.0 - alpha_squared) * (NdotV * NdotV))));

    float specular_brdf = visibility * distribution;

    vec3 diffuse_brdf = (1 / PI) * base_color.rgb;

    // f0 - base color here
    vec3 conductor_fresnel = specular_brdf * (base_color.rgb + (1.0 - base_color.rgb) * pow(1.0 - abs(HdotV), 5));

    float f0 = 0.04;
    float fr = f0 + (1 - f0) * pow(1 - abs(HdotV), 5);
    vec3 fresnel_mix = mix(diffuse_brdf, vec3(specular_brdf), fr);

    vec3 material_color = mix(fresnel_mix, conductor_fresnel, metalness);
    material_color.rgb = encode_srgb(material_color.rgb);

    out_FragColor = vec4(material_color, base_color.a);
    // out_FragColor = vec4(diffuseTexture.rgb, base_color.a);
}
