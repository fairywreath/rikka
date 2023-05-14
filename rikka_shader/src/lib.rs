pub mod compiler;
pub mod reflect;
pub mod types;

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::*;
    use types::ShaderStageType;

    // #[test]
    // fn test_parse_includes() {
    // let source_file_name = "../shaders/scene.glsl";
    // let final_shader_source = read_shader_source_file_with_includes(source_file_name).unwrap();
    // println!("{}", final_shader_source);
    // }

    #[test]
    fn test_compile_shaders_with_includes() {
        // let source_file_name = "../shaders/gbuffer.mesh.glsl";
        // let destination_file_name = "../shaders/gbuffer.mesh.glsl.spv";
        // let test_compile = compile_shader_through_glslangvalidator_cli(
        //     source_file_name,
        //     destination_file_name,
        //     ShaderStageType::Mesh,
        // )
        // .unwrap();

        // let source_file_name = "../shaders/gbuffer.frag.glsl";
        // let destination_file_name = "../shaders/gbuffer.frag.glsl.spv";
        // let test_compile = compile_shader_through_glslangvalidator_cli(
        //     source_file_name,
        //     destination_file_name,
        //     ShaderStageType::Fragment,
        // )
        // .unwrap();

        // let source_file_name = "../shaders/pbr_lighting.frag.glsl";
        // let destination_file_name = "../shaders/pbr_lighting.frag.glsl.spv";
        // let test_compile = compile_shader_through_glslangvalidator_cli(
        //     source_file_name,
        //     destination_file_name,
        //     ShaderStageType::Fragment,
        // )
        // .unwrap();

        let source_file_name = "../shaders/pbr_lighting.vert.glsl";
        let destination_file_name = "../shaders/pbr_lighting.vert.glsl.spv";
        let test_compile = compile_shader_through_glslangvalidator_cli(
            source_file_name,
            destination_file_name,
            ShaderStageType::Vertex,
        )
        .unwrap();
    }
}
