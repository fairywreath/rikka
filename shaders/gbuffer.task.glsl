#version 460 core

#extension GL_NV_mesh_shader : require

layout(local_size_x = 32, local_size_y = 1, local_size_z = 1) in;

out taskNV block
{
    uint meshlet_indices[32];
};

void main()
{
    uint thread_index = gl_LocalInvocationID.x;
    uint meshlet_global_index = gl_WorkGroupID.x;
    uint meshlet_index = (meshlet_global_index * 32) + thread_index;

    meshlet_indices[thread_index] = meshlet_index;

    if (thread_index == 0)
    {
        gl_TaskCountNV = 32;
    }
}
