pub mod loader;
pub mod pass;
pub mod renderer;
pub mod scene;
pub mod scene_renderer;

#[cfg(test)]
mod tests {
    use crate::loader::technique::Technique;

    #[test]
    fn test_parse_technique() {
        let file_name = "../data/simple_pbr.json";
        let file_contents = std::fs::read_to_string(file_name).unwrap();

        let technique: Technique = serde_json::from_str(file_contents.as_str()).unwrap();
        println!("{:#?}", technique);
    }
}
