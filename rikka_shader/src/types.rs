#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShaderStageType {
    Vertex,
    Fragment,
    Geometry,
    Compute,
    Mesh,
    Task,
}

impl ShaderStageType {
    pub fn to_glslang_compiler_extension(&self) -> String {
        match self {
            Self::Vertex => String::from("vert"),
            Self::Fragment => String::from("frag"),
            Self::Geometry => String::from("geom"),
            Self::Compute => String::from("comp"),
            Self::Mesh => String::from("mesh"),
            Self::Task => String::from("task"),
        }
    }

    pub fn to_glslang_stage_defines(&self) -> String {
        match self {
            Self::Vertex => String::from("VERTEX"),
            Self::Fragment => String::from("FRAGMENT"),
            Self::Geometry => String::from("GEOMETRY"),
            Self::Compute => String::from("COMPUTE"),
            Self::Mesh => String::from("MESH"),
            Self::Task => String::from("TASK"),
        }
    }
}

pub struct ShaderData {
    pub bytes: Vec<u8>,
}
