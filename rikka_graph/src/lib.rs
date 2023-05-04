pub mod builder;
pub mod graph;
pub mod parser;
pub mod types;

use types::*;

#[cfg(test)]
mod tests {
    use rikka_gpu::types::RenderPassOperation;

    use super::*;

    #[test]
    fn test_parse() {
        let input = parser::Input {
            resource_type: ResourceType::Attachment,
            name: String::from("gbuffer_colour"),
        };

        let image = parser::ImageDesc {
            format: 32,
            resolution: [1280, 800],
            load_op: RenderPassOperation::Load,
        };

        let output = parser::Output {
            resource_type: ResourceType::Attachment,
            name: String::from("gbuffer_colour"),
            image: Some(image),
        };

        let main_pass = parser::Pass {
            name: String::from("gbuffer_pass"),
            inputs: vec![input],
            outputs: vec![output],
        };

        let graph = parser::Graph {
            name: String::from("main_graph"),
            passes: vec![main_pass],
        };

        let graph_json = serde_json::to_string_pretty(&graph).unwrap();
        // println!("{}", graph_json);

        let deferred_graph = parser::parse_from_file("../data/deferred_graph.json").unwrap();

        // for node in &deferred_graph.nodes {
        // println!{
        //     "{:?}",
        //     deferred_graph.builder.access_node_by_handle(node).unwrap(),
        // );
        // }
    }
}
