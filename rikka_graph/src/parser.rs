use serde::{Deserialize, Serialize};

use anyhow::{Error, Result};

use crate::{builder::*, types::*};

// pub(crate) trait ConvertInfo<T>: Sized {
//     fn convert_into(&self) -> Result<T> {
//         Err(anyhow::anyhow!("Unsupported reflect format conversion!"))
//     }
// }

// impl ConvertInfo<ResourceType> for &str {
//     fn convert_into(&self) -> Result<ResourceType> {
//         match self.as_ref() {
//             "buffer" => Ok(ResourceType::Buffer),
//             "texture" => Ok(ResourceType::Texture),
//             "attachment" => Ok(ResourceType::Attachment),
//             "reference" => Ok(ResourceType::Reference),
//             _ => Err(anyhow::anyhow!(
//                 "Unknown string identifier for ResourceType"
//             )),
//         }
//     }
// }
