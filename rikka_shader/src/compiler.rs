use std::{fs, fs::File, io::prelude::*, process::Command};

use anyhow::{Context, Result};

use crate::types::*;

pub fn read_shader_binary_file(file_name: &str) -> Result<ShaderData> {
    let bytes = fs::read(file_name)?;
    Ok(ShaderData { bytes })
}

pub fn read_shader_source_file(file_name: &str) -> Result<String> {
    let source_string =
        fs::read_to_string(file_name).context("Failed to read shader source file!")?;

    Ok(source_string)
}

pub fn compile_shader_through_glslangvalidator_cli(
    source_file_name: &str,
    destination_file_name: &str,
    shader_type: ShaderStageType,
) -> Result<ShaderData> {
    let shader_source = read_shader_source_file(source_file_name)?;

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
        log::error!(
            "glslangValidator returned error: {:?}",
            command_output.stderr
        );

        Err(anyhow::anyhow!(
            "Failed to compile shader through glslangvalidator!"
        ))
    }
}
