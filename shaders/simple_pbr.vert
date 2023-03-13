#version 460 core

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 texCoord0;
// layout(location = 2) in vec3 normal;
// layout(location = 4) in vec4 tangent;

layout(location = 0) out vec3 out_Position;
layout(location = 1) out vec2 out_TexCoord0;

layout(std140, binding = 0) uniform UniformData
{
    mat4 model;
    mat4 view;
    mat4 proj;
};

void main()
{
    gl_Position = proj * view * model * vec4(position.x, -position.y, position.z, 1.0);

    out_Position = position;
    out_TexCoord0 = texCoord0;
}