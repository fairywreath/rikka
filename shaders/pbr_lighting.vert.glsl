#version 460 core

#pragma shader_stage(vertex)

layout(location = 0) out vec2 vTexcoord0;

void main()
{
    vTexcoord0.xy = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    gl_Position = vec4(vTexcoord0.xy * 2.0f - 1.0f, 0.0f, 1.0f);
}
