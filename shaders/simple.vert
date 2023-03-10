#version 460 core

struct VertexData
{
	float x;
	float y;
	float z;

	float u;
	float v;
};

layout(std430, binding = 0) readonly buffer Vertices
{
	VertexData data[];
} inVertices;

void main()
{
    VertexData vertex = inVertices.data[gl_VertexIndex];
    vec3 pos = vec3(vertex.x, vertex.y, vertex.z);

	gl_Position = vec4(pos, 1.0);
}