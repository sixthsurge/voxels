use std::{collections::HashMap, path::Path};

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct BlockId(String);

#[derive(Clone, Debug)]
pub struct Block {
    pub model: BlockModel,
}

impl Block {
    /// Create a `Block` from a `BlockConfig`
    /// If not already present, any image names referenced in the config are added to `image_names`
    /// as they appear in the new `BlockModel`
    pub fn from_config(
        config: BlockConfig,
        image_names: &mut Vec<String>,
    ) -> Result<Self, BlockConfigError> {
        let model = BlockModel::from_config(config.model, image_names)?;
        Ok(Self { model })
    }
}

#[derive(Clone, Debug)]
pub struct BlockRegistry {
    blocks: HashMap<BlockId, Block>,
    /// Stores the image file name for each texture index
    image_names: Vec<String>,
}

impl BlockRegistry {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            image_names: Vec::new(),
        }
    }

    pub fn get(&self, id: &BlockId) -> &Block {
        self.blocks
            .get(id)
            .expect(&format!("block ID {:?} does not exist", id))
    }

    pub fn add(&mut self, id: &BlockId, block_config: BlockConfig) {
        let block = match Block::from_config(block_config, &mut self.image_names) {
            Ok(r) => r,
            Err(e) => {
                log::error!("failed to create block {} from block config: {}", &id.0, e);
                return;
            }
        };
        self.blocks.insert(id.clone(), block);
    }

    pub fn add_from_toml_str(&mut self, id: &BlockId, toml_str: &str) {
        let config: BlockConfig = match toml::from_str(toml_str) {
            Ok(r) => r,
            Err(e) => {
                log::error!("failed to create block {} from toml string: {}", &id.0, e);
                return;
            }
        };

        self.add(id, config);
    }

    pub fn add_from_toml_file(&mut self, path: &Path) {
        let Some(file_stem) = path.file_stem() else {
            log::error!("failed to create block from toml file: file has no name?");
            return;
        };
        let file_stem = file_stem.to_str().unwrap();

        let Ok(toml_string) = std::fs::read_to_string(path) else {
            log::error!(
                "failed to create block {} from toml file: failed to read file",
                file_stem
            );
            return;
        };

        self.add_from_toml_str(&BlockId(file_stem.into()), &toml_string);
    }

    pub fn add_all_in_dir(&mut self, dir: &Path) {
        std::fs::read_dir(dir)
            .expect(&format!("could not read directory: {:?}", &dir))
            .filter_map(|result| result.ok())
            .for_each(|dir_entry| {
                let path = dir_entry.path();
                let metadata = dir_entry.metadata().unwrap();
                if metadata.is_dir() {
                    self.add_all_in_dir(&path);
                } else {
                    self.add_from_toml_file(&path);
                }
            });
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct BlockConfig {
    model: BlockModelConfig,
}

#[derive(Debug, serde::Deserialize)]
pub struct BlockModelConfig {
    /// Kind of block model
    kind: String,
    /// Texture to use for all faces
    texture: Option<String>,
    /// Texture to use for the positive X texture
    texture_pos_x: Option<String>,
    /// Texture to use for the positive Y texture
    texture_pos_y: Option<String>,
    /// Texture to use for the positive Z texture
    texture_pos_z: Option<String>,
    /// Texture to use for the negative X texture
    texture_neg_x: Option<String>,
    /// Texture to use for the negative Y texture
    texture_neg_y: Option<String>,
    /// Texture to use for the negative Z texture
    texture_neg_z: Option<String>,
    /// Texture to use for the top (positive Y) texture
    texture_top: Option<String>,
    /// Texture to use for the side textures
    texture_sides: Option<String>,
    /// Texture to use for the bottom (negative Y) texture
    texture_bottom: Option<String>,
}

#[derive(Clone, Debug)]
pub enum BlockModel {
    Empty,
    Full(FullBlockModel),
}

impl BlockModel {
    /// Create a `BlockModel` from a `BlockModelConfig`
    /// If not already present, any image names referenced in the config are added to `image_names`
    /// as they appear in the new `BlockModel`
    pub fn from_config(
        config: BlockModelConfig,
        image_names: &mut Vec<String>,
    ) -> Result<Self, BlockConfigError> {
        match config.kind.as_str() {
            "empty" => Ok(Self::Empty),
            "full" => {
                let face_textures = [0; 6];
                Ok(Self::Full(FullBlockModel { face_textures }))
            }
            _ => Err(BlockConfigError::UnexpectedModelKind(config.kind.clone())),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FullBlockModel {
    face_textures: [u32; 6],
}

#[derive(thiserror::Error, Debug)]
pub enum BlockConfigError {
    #[error("unexpected value for model.kind: {0}")]
    UnexpectedModelKind(String),
    #[error("missing texture")]
    MissingTexture,
}

/// If `vec` does not contain `val`, pushes `val` onto `vec` and returns the index of `val` in `vec`
/// Otherwise just returns the index of `val` in `vec`
fn add_or_find<T: Eq>(vec: &mut Vec<T>, val: T) -> usize {
    if let Some(index) = vec.iter().position(|x| *x == val) {
        index
    } else {
        vec.push(val);
        vec.len() - 1
    }
}
