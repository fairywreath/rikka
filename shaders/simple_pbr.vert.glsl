#version 460 core

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 texCoord0;
layout(location = 2) in vec3 normal;
layout(location = 3) in vec4 tangent;

layout(location = 0) out vec3 out_Position;
layout(location = 1) out vec2 out_TexCoord0;
layout(location = 2) out vec3 out_Normal;
layout(location = 3) out vec4 out_Tangent;

layout(std140, binding = 0) uniform SceneUniform
{
    mat4 view;
    mat4 proj;

    vec4 eye_position;

    vec4 light_position;
    float light_range;
    float light_intensity;
};

layout(std140, binding = 1) uniform MeshUniform
{
    mat4 model;
    mat4 inverse_model;

    vec4 baseColorFactor;

    // Indices to the global bindless texture array
    uint diffuseTextureIndex;
    uint metallicRoughnessTextureIndex;
    uint normalTextureIndex;
    uint occlusionTextureIndex;

    // vec4 metallicRoughnessOcclusionFactor;

    // float alpha_cutoff;
    // uint flags;
};

void main()
{
    gl_Position = proj * view * model * vec4(position, 1.0);

    out_Position = (model * vec4(position, 1.0)).xyz;
    out_TexCoord0 = texCoord0;
    out_Normal = mat3(inverse(model)) * normal;
    out_Tangent = tangent;
}