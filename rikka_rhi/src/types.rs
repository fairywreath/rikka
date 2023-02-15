use bitflags::bitflags;

pub enum PipelineStage {
    DrawIndirect,
    VertexInput,
    VertexShader,
    FragmentShader,
    RenderTarget,
    ComputeShader,
    Transfer,
}

pub enum ResourceUsageType {
    Immutable,
    Dynamic,
    Stream,
    Staging,
}
