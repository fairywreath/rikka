use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    process::Command,
};

use anyhow::{Context, Result};

use crate::types::*;

const GLSL_VERSION_DIRECTIVE: &str = "#version 460 core";
const SHADER_INCLUDE_PRAGMA: &str = "#pragma RIKKA_REQUIRE";

pub fn read_shader_binary_file(file_name: &str) -> Result<ShaderData> {
    let bytes = fs::read(file_name)?;
    Ok(ShaderData { bytes })
}

pub fn process_includes(content: &str, base_path: &str, output: &mut String) -> Result<()> {
    for line in content.lines() {
        let trimmed_line = line.trim();

        if trimmed_line.starts_with(SHADER_INCLUDE_PRAGMA) {
            let start_index = trimmed_line.find('(').unwrap_or(trimmed_line.len());
            let end_index = trimmed_line.rfind(')').unwrap_or(start_index);
            let include_path = &trimmed_line[start_index + 1..end_index];

            let include_content = fs::read_to_string(format!("{}/{}", base_path, include_path))?;

            process_includes(include_content.as_str(), base_path, output)?
        } else if trimmed_line == GLSL_VERSION_DIRECTIVE {
            // XXX: Handle error case where version is different
            continue;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }

    Ok(())
}

pub fn read_shader_source_file(file_name: &str) -> Result<String> {
    let source_string =
        fs::read_to_string(file_name).context("Failed to read shader source file!")?;
    Ok(source_string)
}

pub fn read_shader_source_file_with_includes(file_name: &str) -> Result<String> {
    let input_base_path = Path::new(file_name)
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_str()
        .unwrap();

    let initial_shader_source = read_shader_source_file(file_name)?;

    let mut final_shader_source = String::from(GLSL_VERSION_DIRECTIVE);
    process_includes(
        initial_shader_source.as_str(),
        input_base_path,
        &mut final_shader_source,
    )?;

    Ok(final_shader_source)
}

pub fn compile_shader_through_glslangvalidator_cli(
    source_file_name: &str,
    destination_file_name: &str,
    shader_type: ShaderStageType,
) -> Result<ShaderData> {
    let shader_source = read_shader_source_file_with_includes(source_file_name)?;

    let temp_file_name = "temp_shader";
    {
        let mut temp_file = File::create(temp_file_name)?;
        temp_file.write_all(shader_source.as_bytes())?;
    }

    let command_name = match std::env::consts::OS {
        "windows" => "glslangvalidator.exe",
        "linux" => "glslangValidator",
        _ => "glslangValidator",
    };

    let command_output = Command::new(command_name)
        .arg(temp_file_name)
        .arg("-V")
        .args(["--target-env", "vulkan1.3"])
        .args(["-o", destination_file_name])
        .args(["-S", shader_type.to_glslang_compiler_extension().as_str()])
        .args(["--D", shader_type.to_glslang_stage_defines().as_str()])
        .output()?;

    fs::remove_file(temp_file_name).context("Failed to remove temp shader source file")?;

    if command_output.status.success() {
        let shader_data = read_shader_binary_file(destination_file_name)?;
        Ok(shader_data)
    } else {
        // log::error!(
        //     "glslangValidator returned error: {:?}",
        //     command_output.stderr
        // );

        println!(
            "glslangValidator returned error: {:?}",
            String::from_utf8(command_output.stdout)
        );

        Err(anyhow::anyhow!(
            "Failed to compile shader through glslangvalidator!"
        ))
    }
}
