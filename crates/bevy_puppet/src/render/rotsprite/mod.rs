//! RotSprite: a sprite rotated in even steps, each drawn from the sprite scaled
//! up with Scale2x three times and sampled back down, so its pixel art stays
//! crisp, its colours its own, at any angle. The steps sit side by side in one
//! image, with an atlas to show any one of them. Done once, on the CPU, for
//! sprites that turn often and stay the same: a limb, a weapon.
#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "pixel coordinates and step counts, all within an image's size: far below where f32 loses integers"
)]

mod pixels;

use std::f32::consts::TAU;

use bevy::image::{Image, TextureAtlasLayout};
use bevy::math::{BVec2, URect, UVec2, Vec2};

use self::pixels::Pixels;

/// A sprite ready to rotate: scaled up eight times with Scale2x, and the frame
/// its steps are drawn in, square about the pivot.
#[derive(Debug, Clone)]
pub(crate) struct RotSprite {
    large: Pixels,
    frame: UVec2,
    pivot: Vec2,
}

impl RotSprite {
    /// How much larger the sprite is sampled from: Scale2x three times.
    const SCALE: u32 = 8;

    /// The sprite in `rect` of `image`, mirrored as `flip` says, to turn about
    /// `pivot`: in pixels from the rect's top left, down positive. `None` if
    /// `image` can't be read there.
    #[must_use]
    pub(crate) fn new(image: &Image, rect: URect, pivot: Vec2, flip: BVec2) -> Option<Self> {
        Some(Self::from_pixels(
            &Pixels::read(image, rect)?.flipped(flip),
            pivot,
        ))
    }

    fn from_pixels(sprite: &Pixels, pivot: Vec2) -> Self {
        let large = (0..3).fold(sprite.clone(), |pixels, _| pixels.scale2x());
        let size = Vec2::new(sprite.width as f32, sprite.height as f32);
        let reach = [Vec2::ZERO, Vec2::X * size.x, Vec2::Y * size.y, size]
            .into_iter()
            .map(|corner| corner.distance(pivot))
            .fold(0.0, f32::max);
        let span = ((2.0 * reach).ceil() as u32).max(1);
        // Each side as odd or even as twice the pivot's place along it, so
        // the frame's middle falls where the pivot does: on a pixel's edge,
        // or in its middle.
        let parity = |twice: f32| {
            span + u32::from((i64::from(span) + twice.round() as i64).rem_euclid(2) == 1)
        };
        let frame = UVec2::new(parity(2.0 * pivot.x), parity(2.0 * pivot.y));
        Self {
            large,
            frame,
            pivot,
        }
    }

    /// One step's size: square about the pivot, but for its parity.
    #[must_use]
    pub(crate) fn frame(&self) -> UVec2 {
        self.frame
    }

    /// The sprite turned in `steps` even steps from 0, counterclockwise, side
    /// by side: step `n` turned `n / steps` of the way round.
    #[must_use]
    pub(crate) fn strip(&self, steps: u32) -> Image {
        self.pixels(steps).image()
    }

    /// The atlas showing one step of [`strip`](Self::strip) at a time.
    #[must_use]
    pub(crate) fn atlas(&self, steps: u32) -> TextureAtlasLayout {
        TextureAtlasLayout::from_grid(self.frame, steps.max(1), 1, None, None)
    }

    /// The angle step `step` of `steps` is drawn at, in radians.
    #[must_use]
    pub(crate) fn angle(steps: u32, step: u32) -> f32 {
        TAU * step as f32 / steps.max(1) as f32
    }

    fn pixels(&self, steps: u32) -> Pixels {
        let steps = steps.max(1);
        let mut strip = Pixels::new(self.frame.x * steps, self.frame.y);
        for step in 0..steps {
            self.turn(Self::angle(steps, step), &mut strip, step * self.frame.x);
        }
        strip
    }

    /// Draws the sprite turned `angle` into `strip`, from column `left`: each
    /// pixel from where it came from, turned back the other way.
    fn turn(&self, angle: f32, strip: &mut Pixels, left: u32) {
        let (sin, cos) = angle.sin_cos();
        let half = self.frame.as_vec2() / 2.0;
        let scale = Self::SCALE as f32;
        for y in 0..self.frame.y {
            for x in 0..self.frame.x {
                // Each pixel's middle, from the frame's, up positive.
                let (dx, dy) = (x as f32 + 0.5 - half.x, half.y - (y as f32 + 0.5));
                let (sx, sy) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                let (lx, ly) = ((self.pivot.x + sx) * scale, (self.pivot.y - sy) * scale);
                if lx < 0.0 || ly < 0.0 {
                    continue;
                }
                let (lx, ly) = (lx as u32, ly as u32);
                if lx >= self.large.width || ly >= self.large.height {
                    continue;
                }
                strip.set(left + x, y, self.large.get(lx, ly));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;

    const RED: [u8; 4] = [255, 0, 0, 255];

    /// A line from the middle of a 9 by 9 sprite, rightwards.
    fn line() -> Pixels {
        let mut sprite = Pixels::new(9, 9);
        for x in 4..9 {
            sprite.set(x, 4, RED);
        }
        sprite
    }

    fn middle() -> Vec2 {
        Vec2::splat(4.5)
    }

    fn frame(strip: &Pixels, side: u32, step: u32) -> impl Fn(u32, u32) -> [u8; 4] + '_ {
        move |x, y| strip.get(step * side + x, y)
    }

    #[test]
    fn draws_each_frame_twice_its_reach() {
        let rotsprite = RotSprite::from_pixels(&line(), middle());
        // 9 by 9 about its middle: twice its farthest corner, 12.7,
        // rounded up.
        assert_eq!(rotsprite.frame(), UVec2::splat(13));
        let strip = rotsprite.pixels(4);
        assert_eq!((strip.width, strip.height), (52, 13));
    }

    #[test]
    fn turns_counterclockwise_about_the_pivot() {
        let rotsprite = RotSprite::from_pixels(&line(), middle());
        let side = rotsprite.frame().x;
        let strip = rotsprite.pixels(4);
        // Unturned, the line runs right from the middle (6, 6).
        assert_eq!(frame(&strip, side, 0)(9, 6), RED);
        // A quarter turn: up.
        assert_eq!(frame(&strip, side, 1)(6, 3), RED);
        assert_eq!(frame(&strip, side, 1)(9, 6)[3], 0);
        // A half turn: left.
        assert_eq!(frame(&strip, side, 2)(3, 6), RED);
    }

    #[test]
    fn a_pivot_off_the_sprite_keeps_its_parity() {
        // A joint 1.5 left of the sprite: twice that, odd, so an odd frame
        // with its middle in a pixel's middle.
        let rotsprite = RotSprite::from_pixels(&line(), Vec2::new(-1.5, 4.5));
        assert_eq!(rotsprite.frame().x % 2, 1);
    }

    #[test]
    fn keeps_the_sprites_own_colours() {
        let strip = RotSprite::from_pixels(&line(), middle()).pixels(7);
        assert!(strip.data.iter().all(|p| *p == RED || p[3] == 0));
    }

    #[test]
    fn draws_step_n_at_its_share_of_a_turn() {
        assert_eq!(RotSprite::angle(32, 8), FRAC_PI_2);
    }

    #[test]
    fn reads_and_writes_images() {
        let image = line().image();
        let rect = URect::new(0, 0, 9, 9);
        let rotsprite = RotSprite::new(&image, rect, middle(), BVec2::FALSE).unwrap();
        let strip = rotsprite.strip(4);
        assert_eq!(strip.width(), 52);
        assert_eq!(rotsprite.atlas(4).textures.len(), 4);
    }
}
