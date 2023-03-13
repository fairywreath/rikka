#version 460 core

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 texCoord0;

layout(location = 0) out vec4 out_FragColor;

layout(binding = 1) uniform sampler2D diffuseTexture;

void main()
{
    vec3 texture_color = texture(diffuseTexture, texCoord0).xyz;
    out_FragColor = vec4(texture_color, 1.0);
}