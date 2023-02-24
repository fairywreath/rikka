#version 460 core

struct VertexData
{
	float x;
	float y;
	float z;
};

//layout(binding = 0) uniform UniformBuffer
//{
//	mat4 mvp;
//} ubo;

layout(binding = 0) readonly buffer Vertices
{
	VertexData data[];
} inVertices;

void main()
{
    VertexData vertex = inVertices.data[gl_VertexIndex];
    vec3 pos = vec3(vertex.x, vertex.y, vertex.z);

	gl_Position = vec4(pos, 1.0);
//    gl_Position = ubo.mvp * vec4(pos, 1.0);
}