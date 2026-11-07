//! Sprites under bones that turn in steps, pre-rotated with RotSprite: each
//! shows the step nearest its bone's angle, upright in the skeleton, pinned at
//! the joint, so pixel art stays crisp as it turns.

mod rotsprite;

use std::collections::HashMap;

use bevy::math::BVec2;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use self::rotsprite::RotSprite;
use crate::PuppetSystems;
use crate::pose::Placement;
use crate::rig::{Rig, Turn};
use crate::skeleton::{self, Bone, Puppet};

/// A sprite now showing its bone's pre-rotated steps, and what it was, to step
/// again from (its bone's steps changed, or its image) or go back to (its bone
/// no longer turns in steps).
#[derive(Component, Debug, Clone)]
pub(crate) struct Stepped {
    steps: u32,
    strip: Handle<Image>,
    /// One step's size, as it's drawn.
    frame: Vec2,
    was: Was,
}

/// A sprite as the game made it, before it was stepped.
#[derive(Debug, Clone)]
struct Was {
    image: Handle<Image>,
    atlas: Option<TextureAtlas>,
    rect: Option<Rect>,
    custom_size: Option<Vec2>,
    flip: BVec2,
    anchor: Anchor,
    transform: Transform,
}

/// Sprites pre-rotated so far, by what was turned: made once however many bones
/// show them.
#[derive(Resource, Debug, Default)]
pub(crate) struct Steps(HashMap<Key, Made>);

/// What a strip of steps is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    image: AssetId<Image>,
    rect: URect,
    steps: u32,
    pivot: [u32; 2],
    flip: [bool; 2],
}

/// A strip of steps, and one step's size.
#[derive(Debug, Clone)]
struct Made {
    image: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
    frame: UVec2,
}

pub(crate) fn build(app: &mut App) {
    app.init_resource::<Steps>().add_systems(
        PostUpdate,
        (step, show)
            .chain()
            .after(skeleton::place)
            .in_set(PuppetSystems),
    );
}

/// What stepping a sprite reads and writes of it.
type Stepping = (
    Entity,
    &'static ChildOf,
    &'static mut Sprite,
    &'static mut Anchor,
    &'static mut Transform,
    Option<&'static Stepped>,
);

/// Pre-rotates each sprite under a bone that turns in steps, about the joint:
/// its offset from the bone, less its anchor, is where its middle is. Steps it
/// again as its bone's steps or its image change, and puts it back as it was
/// once its bone no longer turns in steps.
fn step(
    mut sprites: Query<Stepping>,
    bones: Query<&Bone>,
    puppets: Query<&Puppet>,
    rigs: Res<Assets<Rig>>,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut steps: ResMut<Steps>,
    mut commands: Commands,
) {
    for (entity, parent, mut sprite, mut anchor, mut transform, stepped) in &mut sprites {
        let Ok(bone) = bones.get(parent.parent()) else {
            continue;
        };
        let turn = puppets
            .get(bone.puppet())
            .ok()
            .and_then(|puppet| rigs.get(&puppet.0))
            .and_then(|rig| rig.turn(bone.index()));
        let was = match (turn, stepped) {
            (None, None) => continue,
            (None, Some(stepped)) => {
                restore(&stepped.was, &mut sprite, &mut anchor, &mut transform);
                commands.entity(entity).remove::<Stepped>();
                continue;
            }
            (Some(turn), Some(stepped))
                if turn.steps == stepped.steps && sprite.image == stepped.strip =>
            {
                continue;
            }
            // Its image changed: the new one, as the old one was placed.
            (Some(_), Some(stepped)) if sprite.image != stepped.strip => Was {
                image: sprite.image.clone(),
                atlas: sprite.texture_atlas.clone(),
                rect: sprite.rect,
                ..stepped.was.clone()
            },
            (Some(_), Some(stepped)) => stepped.was.clone(),
            (Some(_), None) => Was {
                image: sprite.image.clone(),
                atlas: sprite.texture_atlas.clone(),
                rect: sprite.rect,
                custom_size: sprite.custom_size,
                flip: BVec2::new(sprite.flip_x, sprite.flip_y),
                anchor: *anchor,
                transform: *transform,
            },
        };
        let Some(turn) = turn else {
            continue;
        };
        let Some(made) = made(&was, turn, &mut images, &mut layouts, &mut steps) else {
            continue;
        };
        // Drawn as large as it was: its steps scaled as its art was.
        let scale = was.custom_size.map_or(Vec2::ONE, |size| {
            size / rect_of(&was, &images, &layouts).map_or(size, |rect| rect.size().as_vec2())
        });
        let frame = made.frame.as_vec2() * scale;
        sprite.image = made.image.clone();
        sprite.texture_atlas = Some(TextureAtlas {
            layout: made.layout,
            index: 0,
        });
        sprite.rect = None;
        sprite.flip_x = false;
        sprite.flip_y = false;
        sprite.custom_size = was.custom_size.map(|_| frame);
        *anchor = Anchor::CENTER;
        commands.entity(entity).insert(Stepped {
            steps: turn.steps,
            strip: made.image,
            frame,
            was,
        });
    }
}

/// Where in `image` the sprite `was` shows: its atlas frame, its rect, or all
/// of it.
fn rect_of(
    was: &Was,
    images: &Assets<Image>,
    layouts: &Assets<TextureAtlasLayout>,
) -> Option<URect> {
    match &was.atlas {
        Some(atlas) => layouts
            .get(&atlas.layout)
            .and_then(|layout| layout.textures.get(atlas.index).copied()),
        None => was.rect.map(|rect| rect.as_urect()).or_else(|| {
            images
                .get(&was.image)
                .map(|image| URect::from_corners(UVec2::ZERO, image.size()))
        }),
    }
}

/// The strip of `turn`'s steps of the sprite `was`, made once.
fn made(
    was: &Was,
    turn: Turn,
    images: &mut Assets<Image>,
    layouts: &mut Assets<TextureAtlasLayout>,
    steps: &mut Steps,
) -> Option<Made> {
    let rect = rect_of(was, images, layouts)?;
    let size = rect.size().as_vec2();
    let scale = was.custom_size.map_or(Vec2::ONE, |drawn| drawn / size);
    // Its middle from the joint, in its own pixels: where its anchor puts
    // it, as large as its art is.
    let middle = (was.transform.translation.truncate() - was.anchor.0 * size * scale) / scale;
    // The joint from the sprite's top left, down positive.
    let pivot = size / 2.0 - Vec2::new(middle.x, -middle.y);
    let key = Key {
        image: was.image.id(),
        rect,
        steps: turn.steps,
        pivot: [pivot.x.to_bits(), pivot.y.to_bits()],
        flip: [was.flip.x, was.flip.y],
    };
    if let Some(made) = steps.0.get(&key) {
        return Some(made.clone());
    }
    let rotsprite = RotSprite::new(images.get(&was.image)?, rect, pivot, was.flip)?;
    let made = Made {
        image: images.add(rotsprite.strip(turn.steps)),
        layout: layouts.add(rotsprite.atlas(turn.steps)),
        frame: rotsprite.frame(),
    };
    steps.0.insert(key, made.clone());
    Some(made)
}

/// Puts a sprite back as the game made it.
fn restore(was: &Was, sprite: &mut Sprite, anchor: &mut Anchor, transform: &mut Transform) {
    sprite.image.clone_from(&was.image);
    sprite.texture_atlas.clone_from(&was.atlas);
    sprite.rect = was.rect;
    sprite.custom_size = was.custom_size;
    sprite.flip_x = was.flip.x;
    sprite.flip_y = was.flip.y;
    *anchor = was.anchor;
    *transform = was.transform;
}

/// Shows each pre-rotated sprite's step for its bone's angle, upright in the
/// skeleton and pinned at the joint: the step turns, not the sprite. A step an
/// odd number of pixels across sits half a pixel off the joint, so its corner
/// is on a whole pixel and it doesn't shimmer as the camera moves.
fn show(
    mut sprites: Query<(&Stepped, &ChildOf, &mut Sprite, &mut Transform)>,
    bones: Query<&Placement, With<Bone>>,
) {
    for (stepped, parent, mut sprite, mut transform) in &mut sprites {
        let Ok(placement) = bones.get(parent.parent()) else {
            continue;
        };
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.index = Turn {
                steps: stepped.steps,
            }
            .step(placement.angle) as usize;
        }
        let half = stepped.frame / 2.0;
        let pin = half - half.floor();
        // The pin is upright, in the skeleton; the sprite's offset is in its
        // bone, turned.
        let offset = Vec2::from_angle(-placement.angle).rotate(pin);
        transform.translation = offset.extend(stepped.was.transform.translation.z);
        transform.rotation = Quat::from_rotation_z(-placement.angle);
        transform.scale = stepped.was.transform.scale;
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::{AssetPlugin, RenderAssetUsages};
    use bevy::image::{ImagePlugin, TextureAtlasPlugin};
    use bevy::transform::TransformPlugin;
    use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

    use super::*;
    use crate::clip::Clip;
    use crate::pose::Animator;
    use crate::testing::{clip, rig};
    use crate::{Bones, PuppetPlugin};

    /// A one-bone puppet on `rig_source`, turned a quarter, with a red `size`
    /// sprite under its bone, `at` and `anchor`.
    fn stage(
        rig_source: &str,
        size: UVec2,
        at: Vec2,
        anchor: Anchor,
    ) -> (App, Entity, Entity, Handle<Image>) {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            ImagePlugin::default(),
            TextureAtlasPlugin,
            PuppetPlugin,
        ));
        let rig = app
            .world_mut()
            .resource_mut::<Assets<Rig>>()
            .add(rig(rig_source).unwrap());
        let turn = app.world_mut().resource_mut::<Assets<Clip>>().add(clip(
            r#"(layer: Move, channels: [(bone: "arm", angle: Keys([(0, 90)]))])"#,
        ));
        let mut animator = Animator::default();
        animator.play(&turn);
        let puppet = app.world_mut().spawn((Puppet(rig), animator)).id();
        app.update();
        let arm = app.world().get::<Bones>(puppet).unwrap().get(0).unwrap();
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new_fill(
                Extent3d {
                    width: size.x,
                    height: size.y,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[255, 0, 0, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::all(),
            ));
        let sprite = app
            .world_mut()
            .spawn((
                Sprite::from_image(image.clone()),
                anchor,
                Transform::from_translation(at.extend(0.0)),
                ChildOf(arm),
            ))
            .id();
        app.update();
        (app, puppet, sprite, image)
    }

    const STEPPED: &str = r#"(bones: {"arm": (turns: 4)})"#;

    #[test]
    fn a_sprite_under_a_stepped_bone_shows_its_step_upright() {
        // Its middle 4 down the joint: an arm hanging from its shoulder.
        let (app, _, sprite, _) = stage(
            STEPPED,
            UVec2::new(4, 8),
            Vec2::new(0.0, -4.0),
            Anchor::CENTER,
        );
        let world = app.world();
        let shown = world.get::<Sprite>(sprite).unwrap();
        assert_eq!(
            shown.texture_atlas.as_ref().unwrap().index,
            1,
            "a quarter of 4 steps"
        );
        let transform = world.get::<Transform>(sprite).unwrap();
        assert_eq!(
            transform.translation.truncate(),
            Vec2::ZERO,
            "pinned at the joint"
        );
        // Turned back by the bone's quarter: upright, the step does the
        // turning.
        let upright = world.get::<GlobalTransform>(sprite).unwrap().rotation();
        assert!(upright.angle_between(Quat::IDENTITY) < 1e-4);
    }

    #[test]
    fn an_odd_step_sits_off_the_joint_by_half_a_pixel() {
        // 3 by 3 about its middle: steps 5 across, odd.
        let (app, puppet, sprite, _) = stage(STEPPED, UVec2::splat(3), Vec2::ZERO, Anchor::CENTER);
        let world = app.world();
        let arm = world.get::<Bones>(puppet).unwrap().get(0).unwrap();
        let joint = world
            .get::<GlobalTransform>(arm)
            .unwrap()
            .translation()
            .truncate();
        let middle = world
            .get::<GlobalTransform>(sprite)
            .unwrap()
            .translation()
            .truncate();
        assert!(
            middle.distance(joint + Vec2::splat(0.5)) < 1e-4,
            "{middle} vs {joint}"
        );
    }

    #[test]
    fn its_anchor_says_where_its_middle_is() {
        // Its top middle at the joint: as the centred one 4 down.
        let (app, _, sprite, _) = stage(STEPPED, UVec2::new(4, 8), Vec2::ZERO, Anchor::TOP_CENTER);
        let world = app.world();
        assert_eq!(*world.get::<Anchor>(sprite).unwrap(), Anchor::CENTER);
        let frame = world.get::<Stepped>(sprite).unwrap().frame;
        assert_eq!(
            frame,
            Vec2::splat(18.0),
            "about the top middle, as the hanging arm's"
        );
    }

    #[test]
    fn a_sprite_goes_back_as_it_was_once_its_bone_no_longer_steps() {
        let (mut app, puppet, sprite, image) = stage(
            STEPPED,
            UVec2::new(4, 8),
            Vec2::new(0.0, -4.0),
            Anchor::CENTER,
        );
        let id = app.world().get::<Puppet>(puppet).unwrap().0.id();
        app.world_mut()
            .resource_mut::<Assets<Rig>>()
            .insert(id, rig(r#"(bones: {"arm": ()})"#).unwrap())
            .unwrap();
        app.update();
        let world = app.world();
        assert!(world.get::<Stepped>(sprite).is_none());
        let shown = world.get::<Sprite>(sprite).unwrap();
        assert_eq!(shown.image, image);
        assert!(shown.texture_atlas.is_none());
        assert_eq!(
            world.get::<Transform>(sprite).unwrap().translation,
            Vec3::new(0.0, -4.0, 0.0)
        );
    }
}
