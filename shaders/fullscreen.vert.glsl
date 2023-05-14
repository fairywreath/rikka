#version 460 core

layout(location = 0) out vec2 out_texCoord;
layout(location = 1) flat out uint out_textureId;

void main()
{
    out_texCoord = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    gl_Position = vec4(out_texCoord.xy * 2.0f - 1.0f, 0.0f, 1.0f);

    out_textureId = gl_InstanceIndex;
}
