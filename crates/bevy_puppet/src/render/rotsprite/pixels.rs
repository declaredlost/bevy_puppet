//! A sprite's pixels, read from an image, scaled with Scale2x, and written back
//! as one.

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "pixel coordinates, clamped within the sprite"
)]
use bevy::asset::RenderAssetUsages;
use bevy::color::ColorToPacked;
use bevy::image::{Image, ImageSampler};
use bevy::math::URect;
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

/// Pixels in rows, top first, each sRGB with alpha.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pixels {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) data: Vec<[u8; 4]>,
}

impl Pixels {
    /// `width` by `height`, clear.
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![[0; 4]; (width * height) as usize],
        }
    }

    /// The pixels in `rect` of `image`, if it can be read there.
    pub(crate) fn read(image: &Image, rect: URect) -> Option<Self> {
        let size = rect.size();
        let mut pixels = Self::new(size.x, size.y);
        for y in 0..size.y {
            for x in 0..size.x {
                let color = image.get_color_at(rect.min.x + x, rect.min.y + y).ok()?;
                pixels.set(x, y, color.to_srgba().to_u8_array());
            }
        }
        Some(pixels)
    }

    pub(crate) fn get(&self, x: u32, y: u32) -> [u8; 4] {
        self.data[(y * self.width + x) as usize]
    }

    pub(crate) fn set(&mut self, x: u32, y: u32, pixel: [u8; 4]) {
        self.data[(y * self.width + x) as usize] = pixel;
    }

    /// The pixel at `(x, y)`, or the nearest edge's off the sprite.
    fn at(&self, x: i64, y: i64) -> [u8; 4] {
        let x = x.clamp(0, i64::from(self.width) - 1) as u32;
        let y = y.clamp(0, i64::from(self.height) - 1) as u32;
        self.get(x, y)
    }

    /// Twice as large, with Scale2x: each pixel four, each corner its two
    /// neighbours' colour where they agree and the other two don't.
    pub(crate) fn scale2x(&self) -> Self {
        let mut out = Self::new(self.width * 2, self.height * 2);
        for y in 0..i64::from(self.height) {
            for x in 0..i64::from(self.width) {
                let p = self.at(x, y);
                let (up, right) = (self.at(x, y - 1), self.at(x + 1, y));
                let (left, down) = (self.at(x - 1, y), self.at(x, y + 1));
                let corner = |a: [u8; 4], b: [u8; 4], c: [u8; 4], d: [u8; 4]| {
                    if a == b && a != c && b != d { a } else { p }
                };
                let (ox, oy) = (x as u32 * 2, y as u32 * 2);
                out.set(ox, oy, corner(left, up, down, right));
                out.set(ox + 1, oy, corner(up, right, left, down));
                out.set(ox, oy + 1, corner(down, left, right, up));
                out.set(ox + 1, oy + 1, corner(right, down, up, left));
            }
        }
        out
    }

    /// As an image, sampled nearest, for the GPU.
    pub(crate) fn image(&self) -> Image {
        let mut image = Image::new(
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            self.data.iter().flatten().copied().collect(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        );
        image.sampler = ImageSampler::nearest();
        image
    }
}
