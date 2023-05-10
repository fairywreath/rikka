#version 460 core

#extension GL_EXT_nonuniform_qualifier : enable

layout(location = 0) in vec2 texCoord;
layout(location = 1) flat in uint textureId;

layout(location = 0) out vec4 out_FragColor;

layout(set = 0, binding = 10) uniform sampler2D globalTextures[];

void main()
{
    out_FragColor = texture(globalTextures[nonuniformEXT(textureId)], texCoord.xy);
}