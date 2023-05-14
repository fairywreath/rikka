#extension GL_EXT_shader_16bit_storage : require
#extension GL_EXT_shader_8bit_storage : require

struct MeshletVertexData
{
    // Normals
    uint8_t nx, ny, nz, nw;

    // Tangents
    uint8_t tx, ty, tz, tw;

    // Texcoords
    float16_t tu, tv;
};

struct MeshletVertexPosition
{
    vec3 vertex;

    uint _pad0;
};

struct Meshlet
{
    vec3 center;
    float radius;

    int8_t cone_axis[3];
    int8_t cone_cutoff;

    // Index to `MeshletVertexData` array
    uint data_offset;

    // Index to `MeshMaterial`? array
    uint mesh_index;

    uint8_t vertex_count;
    uint8_t triangle_count;
};

#define MESHLET_BINDING 5
#define MESHLET_DATA_BINDING 6
#define MESHLET_VERTEX_POSITIONS_BINDING 7
#define MESHLET_VERTEX_DATA_BINDING 8

layout(set = MATERIAL_SET, binding = MESHLET_BINDING) readonly buffer Meshlets
{
    Meshlet meshlets[];
};

layout(set = MATERIAL_SET, binding = MESHLET_DATA_BINDING) readonly buffer MeshletDatas
{
    uint meshlet_datas[];
};

layout(set = MATERIAL_SET, binding = MESHLET_VERTEX_POSITIONS_BINDING) readonly buffer MeshletVertexPositions
{
    MeshletVertexPosition meshlet_vertex_positions[];
};

layout(set = MATERIAL_SET, binding = MESHLET_VERTEX_DATA_BINDING) readonly buffer MeshletVertexDatas
{
    MeshletVertexData meshlet_vertex_datas[];
};
