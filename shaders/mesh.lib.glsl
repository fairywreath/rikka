uint DrawFlags_AlphaMask = 1 << 0;
uint DrawFlags_DoubleSided = 1 << 1;
uint DrawFlags_Transparent = 1 << 2;
uint DrawFlags_HasNormals = 1 << 4;
uint DrawFlags_HasTexCoords = 1 << 5;
uint DrawFlags_HasTangents = 1 << 6;
uint DrawFlags_HasJoints = 1 << 7;
uint DrawFlags_HasWeights = 1 << 8;
uint DrawFlags_AlphaDither = 1 << 9;

struct MeshMaterial
{
    // Texture indices
    uint diffuse_texture_index;
    uint metallic_roughness_texture_index;
    uint normal_texture_index;
    uint occlusion_texture_index;

    vec4 emissive;
    vec4 base_color_factor;
    vec4 metallic_roughness_occlusion_factor;

    uint draw_flags;
    float alpha_cutoff;

    uint vertex_offset;
    uint mesh_index;

    uint meshlet_offset;
    uint meshlet_count;

    uint _pad0;
    uint _pad1;
};

struct MeshInstance
{
    mat4 model;
    mat4 model_inverse;

    uint mesh_draw_index;

    uint _pad0;
    uint _pad1;
    uint _pad2;
};

struct MeshDrawCommand
{
    uint draw_id;

    // VkDrawIndexedIndirectCommand
    uint index_count;
    uint first_index;
    uint vertex_offset;
    uint first_instance;

    // VkDrawMeshTasksIndirectCommandNV
    uint task_count;
    uint first_task;
};

#define MESH_MATERIALS_BINDING 1
#define MESH_DRAW_COMMANDS_BINDING 2
#define MESH_INSTANCES_BINDING 3
#define MESH_BOUNDS_BINDING 4

layout(std430, set = MATERIAL_SET, binding = MESH_MATERIALS_BINDING) readonly buffer MeshMaterials
{
    MeshMaterial mesh_materials[];
};

layout(std430, set = MATERIAL_SET, binding = MESH_DRAW_COMMANDS_BINDING) readonly buffer MeshDrawCommands
{
    MeshDrawCommand mesh_draw_commands[];
};

layout(std430, set = MATERIAL_SET, binding = MESH_INSTANCES_BINDING) readonly buffer MeshInstances
{
    MeshInstance mesh_instances[];
};

layout(std430, set = MATERIAL_SET, binding = MESH_BOUNDS_BINDING) readonly buffer MeshBounds
{
    vec4 mesh_bounds[];
};
