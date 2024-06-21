use std::{fs::File, io::BufReader, ops::Deref, path::Path};

use glam::{UVec2, UVec3};
use image::GenericImageView;
use winit::dpi::PhysicalSize;

/// Format to use when loading images (hardcoded)
pub const IMAGE_FORMAT: image::ImageFormat = image::ImageFormat::Png;

/// Variables that can be chosen when creating a texture
#[derive(Clone, Debug)]
pub struct TextureConfig {
    pub label: wgpu::Label<'static>,
    pub usage: wgpu::TextureUsages,
    pub mip_level_count: u32,
}

impl Default for TextureConfig {
    fn default() -> Self {
        Self {
            label: None,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            mip_level_count: 1,
        }
    }
}

/// Trait for types holding a `wgpu::Texture`
pub trait TextureHolder {
    fn texture(&self) -> &wgpu::Texture;
    fn size(&self) -> UVec3;
    fn view_dimension(&self) -> wgpu::TextureViewDimension;

    /// Create a `wgpu::TextureView` and `wgpu::Sampler` for this texture holder
    /// and wrap them in one object
    fn with_view_and_sampler(
        self,
        device: &wgpu::Device,
        sampler_descriptor: wgpu::SamplerDescriptor<'static>,
    ) -> WithViewAndSampler<Self>
    where
        Self: Sized,
    {
        WithViewAndSampler::wrap(device, self, sampler_descriptor)
    }
}

/// Helper type holding a `wgpu::Texture` that is loaded from an image file
#[derive(Debug)]
pub struct ImageTexture {
    texture: wgpu::Texture,
    size: UVec2,
}

impl ImageTexture {
    /// Create an `ImageTexture` from an existing `image::Image`
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &image::DynamicImage,
        config: &TextureConfig,
    ) -> Self {
        let dim = image.dimensions();
        let extent = wgpu::Extent3d {
            width: dim.0,
            height: dim.1,
            depth_or_array_layers: 1,
        };

        // get pure image data as RGBA8
        let image_data = image.to_rgba8();

        // create texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: config.label,
            size: extent,
            mip_level_count: config.mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb, // assuming all images loaded will by sRGB
            usage: config.usage,
            view_formats: &[],
        });

        // queue the copying of the image data to the texture
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dim.0),
                rows_per_image: Some(dim.1),
            },
            extent,
        );

        Self {
            texture,
            size: UVec2::new(dim.0, dim.1),
        }
    }

    /// Create an image texture from a file
    pub fn from_file(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: impl AsRef<Path>,
        config: &TextureConfig,
    ) -> Result<Self, ImageTextureError> {
        let file = File::open(path).map_err(|e| ImageTextureError::IoError(e))?;
        let reader = BufReader::new(file);
        let image =
            image::load(reader, IMAGE_FORMAT).map_err(|e| ImageTextureError::ImageError(e))?;

        Ok(Self::from_image(device, queue, &image, config))
    }
}

impl TextureHolder for ImageTexture {
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    fn size(&self) -> UVec3 {
        UVec3::new(self.size.x, self.size.y, 1)
    }

    fn view_dimension(&self) -> wgpu::TextureViewDimension {
        wgpu::TextureViewDimension::D2
    }
}

/// Helper type holding a `wgpu::Texture` that used as a texture array
#[derive(Debug)]
pub struct ArrayTexture {
    texture: wgpu::Texture,
    individual_image_size: UVec2,
    layer_count: u32,
}

impl ArrayTexture {
    /// Create an array texture from a slice of existing `image::Image`s
    pub fn from_images(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        images: &[image::DynamicImage],
        config: &TextureConfig,
    ) -> Result<Self, ArrayTextureError> {
        // get individual image size and make sure all images are the same size
        let dim = images
            .first()
            .expect("`images` should not be empty")
            .dimensions();

        for image in images {
            if image.dimensions() != dim {
                return Err(ArrayTextureError::DifferentlySizedImages);
            }
        }

        let layer_count = images.len() as u32;

        // create texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: config.label,
            size: wgpu::Extent3d {
                width: dim.0,
                height: dim.1,
                depth_or_array_layers: layer_count,
            },
            mip_level_count: config.mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb, // assuming all images loaded will by sRGB
            usage: config.usage,
            view_formats: &[],
        });

        for (image_index, image) in images.iter().enumerate() {
            // get pure image data as RGBA8
            let image_data = image.to_rgba8();

            // quee the copying of the image data into texture array
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: image_index as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &image_data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * dim.0),
                    rows_per_image: Some(dim.1),
                },
                wgpu::Extent3d {
                    width: dim.0,
                    height: dim.1,
                    depth_or_array_layers: 1,
                },
            );
        }

        Ok(Self {
            texture,
            individual_image_size: UVec2::new(dim.0, dim.1),
            layer_count,
        })
    }

    /// Create an array texture from a slice of file paths to images
    pub fn from_files(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        paths: &[impl AsRef<Path>],
        _image_format: image::ImageFormat,
        config: &TextureConfig,
    ) -> Result<Self, ArrayTextureError> {
        // load images
        // I decided not to use an iterator for this because I'm not sure how to properly return
        // errors from within the closure, this could be a bit cleaner
        let mut images = Vec::with_capacity(paths.len());
        for path in paths {
            let file = File::open(path).map_err(|e| ArrayTextureError::IoError(e))?;
            let reader = BufReader::new(file);
            images.push(
                image::load(reader, IMAGE_FORMAT).map_err(|e| ArrayTextureError::ImageError(e))?,
            );
        }

        Self::from_images(device, queue, &images, config)
    }
}

impl TextureHolder for ArrayTexture {
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    fn size(&self) -> UVec3 {
        UVec3::new(
            self.individual_image_size.x,
            self.individual_image_size.y,
            self.layer_count,
        )
    }

    fn view_dimension(&self) -> wgpu::TextureViewDimension {
        wgpu::TextureViewDimension::D2Array
    }
}

/// Helper type holding a `wgpu::Texture` that is used as a depth texture
#[derive(Debug)]
pub struct DepthTexture {
    texture: wgpu::Texture,
    format: wgpu::TextureFormat,
    compare_func: wgpu::CompareFunction,
    label: wgpu::Label<'static>,
    size: UVec2,
}

impl DepthTexture {
    pub fn new(
        device: &wgpu::Device,
        window_size: PhysicalSize<u32>,
        format: wgpu::TextureFormat,
        compare_func: wgpu::CompareFunction,
        label: wgpu::Label<'static>,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width: window_size.width,
                height: window_size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        Self {
            texture,
            format,
            compare_func,
            label,
            size: UVec2::new(window_size.width, window_size.height),
        }
    }

    /// Returns a new depth texture that is the same but resized
    /// (for when the window gets resized)
    pub fn recreate(&self, device: &wgpu::Device, new_size: PhysicalSize<u32>) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: self.label,
            size: wgpu::Extent3d {
                width: new_size.width,
                height: new_size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        Self {
            texture,
            format: self.format,
            compare_func: self.compare_func,
            label: self.label,
            size: UVec2::new(new_size.width, new_size.height),
        }
    }
}

impl TextureHolder for DepthTexture {
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    fn view_dimension(&self) -> wgpu::TextureViewDimension {
        wgpu::TextureViewDimension::D2
    }

    fn size(&self) -> UVec3 {
        UVec3::new(self.size.x, self.size.y, 1)
    }
}

/// Wraps a `TextureHolder` with its view and sampler
/// Note: the wrapped `TextureHolder` cannot be accessed mutably while it is stored in the
/// `WithViewAndSampler`. This is because if the texture is recreated, the view and sampler will
/// become invalid. Instead, you must first unwrap the value with `unwrap()`, and then rewrap it
/// in a new `WithViewAndSampler`
#[derive(Debug)]
pub struct WithViewAndSampler<T>
where
    T: TextureHolder,
{
    wrapped: T,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    sampler_descriptor: wgpu::SamplerDescriptor<'static>,
}

impl<T> WithViewAndSampler<T>
where
    T: TextureHolder,
{
    pub fn wrap(
        device: &wgpu::Device,
        wrapped: T,
        sampler_descriptor: wgpu::SamplerDescriptor<'static>,
    ) -> Self {
        let sampler = device.create_sampler(&sampler_descriptor);
        let view = wrapped
            .texture()
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wrapped.view_dimension()),
                ..Default::default()
            });

        Self {
            wrapped,
            view,
            sampler,
            sampler_descriptor,
        }
    }

    pub fn unwrap(self) -> T {
        self.wrapped
    }

    pub fn wrapped(&self) -> &T {
        &self.wrapped
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// returns the sampler descriptor that was used to create the sampler
    /// useful for recreating the sampler without having to remake the sampler descriptor
    pub fn sampler_descriptor(&self) -> &wgpu::SamplerDescriptor<'static> {
        &self.sampler_descriptor
    }
}

impl<T> Deref for WithViewAndSampler<T>
where
    T: TextureHolder,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.wrapped
    }
}

/// errors returned by `ImageTexture::new`
#[derive(Debug, thiserror::Error)]
pub enum ImageTextureError {
    #[error("io error: {0}")]
    IoError(std::io::Error),
    #[error("image error: {0}")]
    ImageError(image::ImageError),
}

/// errors returned by `ArrayTexture::new`
#[derive(Debug, thiserror::Error)]
pub enum ArrayTextureError {
    #[error("io error: {0}")]
    IoError(std::io::Error),
    #[error("image error: {0}")]
    ImageError(image::ImageError),
    #[error("image sizes don't match!")]
    DifferentlySizedImages,
}
