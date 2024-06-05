use std::{fs::File, io::BufReader, path::Path};

use glam::{UVec2, UVec3};
use image::{GenericImageView, ImageFormat};
use wgpu::{Device, TextureUsages, TextureViewDimension};

use super::context::RenderContext;

/// helper struct grouping a `wgpu::Texture` with its corresponding `wgpu::TextureView` and
/// `wgpu::Sampler` and easing the creation of different kinds of texture
pub struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl Texture {
    pub fn new(
        device: &wgpu::Device,
        size: UVec3,
        dimension: wgpu::TextureDimension,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        mip_level_count: u32,
        address_mode: wgpu::AddressMode,
        mag_filter: wgpu::FilterMode,
        min_filter: wgpu::FilterMode,
        mipmap_filter: wgpu::FilterMode,
        label: wgpu::Label,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: size.z,
            },
            mip_level_count,
            sample_count: 1,
            dimension,
            format,
            usage,
            label,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }

    pub fn new_depth_texture(
        device: &Device,
        size: UVec2,
        format: wgpu::TextureFormat,
        label: wgpu::Label,
    ) -> Self {
        let extent = wgpu::Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }

    /// create a texture from a PNG image file
    pub fn load(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: impl AsRef<Path>,
        mip_level_count: u32,
        address_mode: wgpu::AddressMode,
        mag_filter: wgpu::FilterMode,
        min_filter: wgpu::FilterMode,
        mipmap_filter: wgpu::FilterMode,
        label: wgpu::Label,
    ) -> Result<Self, LoadTextureError> {
        let file = File::open(path).map_err(|e| LoadTextureError::IoError(e))?;
        let file_reader = BufReader::new(file);

        let image = image::load(file_reader, ImageFormat::Png)
            .map_err(|e| LoadTextureError::ImageError(e))?;
        let image_rgba = image.to_rgba8();

        let dimensions = image.dimensions();
        let dimensions = UVec2::new(dimensions.0, dimensions.1);

        let extent = wgpu::Extent3d {
            width: dimensions.x,
            height: dimensions.y,
            depth_or_array_layers: 1,
        };

        let ret = Self::new(
            device,
            UVec3::new(dimensions.x, dimensions.y, 1),
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            mip_level_count,
            address_mode,
            mag_filter,
            min_filter,
            mipmap_filter,
            label,
        );

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &ret.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image_rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.x),
                rows_per_image: Some(dimensions.y),
            },
            extent,
        );

        Ok(ret)
    }

    /// create a new array texture from all PNG images in `paths`
    pub fn load_array(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        paths: &[impl AsRef<Path>],
        individual_image_size: UVec2,
        mip_level_count: u32,
        address_mode: wgpu::AddressMode,
        mag_filter: wgpu::FilterMode,
        min_filter: wgpu::FilterMode,
        mipmap_filter: wgpu::FilterMode,
        label: wgpu::Label,
    ) -> Result<Self, LoadTextureArrayError> {
        // create texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: individual_image_size.x,
                height: individual_image_size.y,
                depth_or_array_layers: paths.len() as u32,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            label,
            view_formats: &[],
        });

        // load images
        for (index, path) in paths.iter().enumerate() {
            let file = File::open(path).map_err(|e| LoadTextureArrayError::IoError(e))?;
            let file_reader = BufReader::new(file);

            let image = image::load(file_reader, ImageFormat::Png)
                .map_err(|e| LoadTextureArrayError::ImageError(e))?;
            let image_rgba = image.to_rgba8();

            let dimensions = image.dimensions();
            let dimensions = UVec2::new(dimensions.0, dimensions.1);

            if dimensions != individual_image_size {
                return Err(LoadTextureArrayError::ImageSizeError(
                    path.as_ref()
                        .to_string_lossy()
                        .to_string(),
                    individual_image_size,
                    dimensions,
                ));
            }

            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: index as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &image_rgba,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * dimensions.x),
                    rows_per_image: Some(dimensions.y),
                },
                wgpu::Extent3d {
                    width: individual_image_size.x,
                    height: individual_image_size.y,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
        })
    }

    /// returns the underlying `wgpu::Texture`
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// returns the `wgpu::TextureView` for this texture
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// returns the `wgpu::Sampler` for this texture
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadTextureError {
    #[error("io error: {0}")]
    IoError(std::io::Error),
    #[error("image error: {0}")]
    ImageError(image::ImageError),
}

#[derive(Debug, thiserror::Error)]
pub enum LoadTextureArrayError {
    #[error("io error: {0}")]
    IoError(std::io::Error),
    #[error("image error: {0}")]
    ImageError(image::ImageError),
    #[error("image {0} is wrongly sized: expected {1}, got {2}")]
    ImageSizeError(String, UVec2, UVec2),
}
