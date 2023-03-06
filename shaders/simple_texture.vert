#version 460 core

struct VertexData
{
	float x;
	float y;
	float z;

	// float u;
	// float v;
};

//layout(binding = 0) uniform UniformBuffer
//{
//	mat4 mvp;
//} ubo;

layout(location = 0) out vec2 uv;

layout(std430, binding = 0) readonly buffer Vertices
{
	VertexData data[];
} inVertices;

layout(std140, binding = 2) uniform UniformData {
	mat4 view;
	mat4 proj;
};

void main()
{
    VertexData vertex = inVertices.data[gl_VertexIndex];
    vec3 pos = vec3(vertex.x, vertex.y, vertex.z);

	// gl_Position = vec4(vertex.position, 1.0);
	gl_Position = proj * view * vec4(pos, 1.0);

	// uv = vec2(vertex.u, vertex.v);

//    gl_Position = ubo.mvp * vec4(pos, 1.0);
}