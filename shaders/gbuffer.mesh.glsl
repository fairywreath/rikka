#version 460 core

#pragma RIKKA_REQUIRE(common.lib.glsl)
#pragma RIKKA_REQUIRE(mesh.lib.glsl)
#pragma RIKKA_REQUIRE(meshlet.lib.glsl)
#pragma RIKKA_REQUIRE(scene.lib.glsl)

#extension GL_NV_mesh_shader : require

layout(local_size_x = 32, local_size_y = 1, local_size_z = 1) in;

layout(triangles, max_vertices = 64, max_primitives = 124) out;

in taskNV block
{
    uint meshletIndices[32];
};

layout(location = 0) out vec2 vTexcoord0[];
layout(location = 1) out vec4 vNormal_BiTanX[];
layout(location = 2) out vec4 vTangent_BiTanY[];
layout(location = 3) out vec4 vPosition_BiTanZ[];
layout(location = 4) out flat uint mesh_draw_index[];

uint hash(uint a)
{
    a = (a + 0x7ed55d16) + (a << 12);
    a = (a ^ 0xc761c23c) ^ (a >> 19);
    a = (a + 0x165667b1) + (a << 5);
    a = (a + 0xd3a2646c) ^ (a << 9);
    a = (a + 0xfd7046c5) + (a << 3);
    a = (a ^ 0xb55a4f09) ^ (a >> 16);
    return a;
}

void main()
{
    uint ti = gl_LocalInvocationID.x;
    uint mi = meshletIndices[gl_WorkGroupID.x];

    MeshMaterial mesh_draw = mesh_materials[meshlets[mi].mesh_index];

    uint vertex_count = uint(meshlets[mi].vertex_count);
    uint triangle_count = uint(meshlets[mi].triangle_count);
    uint index_count = triangle_count * 3;

    uint data_offset = meshlets[mi].data_offset;
    uint vertex_offset = data_offset;
    uint index_offset = data_offset + vertex_count;

    bool has_normals = (mesh_draw.draw_flags & DrawFlags_HasNormals) != 0;
    bool has_tangents = (mesh_draw.draw_flags & DrawFlags_HasTangents) != 0;

    float i8_inverse = 1.0 / 127.0;

#if DEBUG
    uint mhash = hash(mi);
    vec3 mcolor = vec3(float(mhash & 255), float((mhash >> 8) & 255), float((mhash >> 16) & 255)) / 255.0;
#endif

    uint mesh_instance_index = mesh_draw_commands[gl_DrawIDARB].draw_id;

    mat4 model = mesh_instances[mesh_instance_index].model;
    mat4 model_inverse = mesh_instances[mesh_instance_index].model_inverse;

    // TODO: if we have meshlets with 62 or 63 vertices then we pay a small penalty for branch divergence here - we can
    // instead redundantly xform the last vertex
    for (uint i = ti; i < vertex_count; i += 32)
    {
        uint vi = meshlet_datas[vertex_offset + i]; // + mesh_draw.vertex_offset;

        vec3 position = vec3(meshlet_vertex_positions[vi].vertex.x, meshlet_vertex_positions[vi].vertex.y,
                             meshlet_vertex_positions[vi].vertex.z);

        if (has_normals)
        {
            vec3 normal = vec3(int(meshlet_vertex_datas[vi].nx), int(meshlet_vertex_datas[vi].ny),
                               int(meshlet_vertex_datas[vi].nz)) *
                              i8_inverse -
                          1.0;
            vNormal_BiTanX[i].xyz = normalize(mat3(model_inverse) * normal);
        }

        if (has_tangents)
        {
            vec3 tangent = vec3(int(meshlet_vertex_datas[vi].tx), int(meshlet_vertex_datas[vi].ty),
                                int(meshlet_vertex_datas[vi].tz)) *
                               i8_inverse -
                           1.0;
            vTangent_BiTanY[i].xyz = normalize(mat3(model) * tangent.xyz);

            vec3 bitangent =
                cross(vNormal_BiTanX[i].xyz, tangent.xyz) * (int(meshlet_vertex_datas[vi].tw) * i8_inverse - 1.0);
            vNormal_BiTanX[i].w = bitangent.x;
            vTangent_BiTanY[i].w = bitangent.y;
            vPosition_BiTanZ[i].w = bitangent.z;
        }

        vTexcoord0[i] = vec2(meshlet_vertex_datas[vi].tu, meshlet_vertex_datas[vi].tv);

        gl_MeshVerticesNV[i].gl_Position = view_projection * (model * vec4(position, 1));

        vec4 worldPosition = model * vec4(position, 1.0);
        vPosition_BiTanZ[i].xyz = worldPosition.xyz / worldPosition.w;

        mesh_draw_index[i] = meshlets[mi].mesh_index;

#if DEBUG
        vColour[i] = vec4(mcolor, 1.0);
#endif
    }

    uint indexGroupCount = (index_count + 3) / 4;

    for (uint i = ti; i < indexGroupCount; i += 32)
    {
        writePackedPrimitiveIndices4x8NV(i * 4, meshlet_datas[index_offset + i]);
    }

    if (ti == 0)
    {
        gl_PrimitiveCountNV = uint(meshlets[mi].triangle_count);
    }
}
