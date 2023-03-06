#version 460 core

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 out_FragColor;

layout(binding = 1) uniform sampler2D _Texture;

void main()
{
    // out_FragColor = texture(_Texture, uv);
    out_FragColor = vec4(1.0, 0.0, 0.0, 1.0);
}