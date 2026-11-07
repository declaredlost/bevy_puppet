//! Sprites under bones that turn in steps, pre-rotated with RotSprite: each
//! shows the step nearest its bone's angle, upright in the skeleton, pinned at
//! the joint, so pixel art stays crisp as it turns.

mod rotsprite;

use std::collections::HashMap;

use bevy::prelude::*;

use self::rotsprite::RotSprite;
use crate::PuppetSystems;
use crate::pose::Placement;
use crate::rig::Rig;
use crate::skeleton::{self, Bone, Puppet};

/// A sprite now showing its bone's pre-rotated steps.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct Stepped {
    /// How many steps it turns in.
    steps: u32,
    /// Its z, over its bone's other sprites.
    z: f32,
}

/// Sprites pre-rotated so far, by what was turned: made once however many bones
/// show them.
#[derive(Resource, Debug, Default)]
pub(crate) struct Steps(HashMap<Key, (Handle<Image>, Handle<TextureAtlasLayout>)>);

/// What a strip of steps is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    image: AssetId<Image>,
    rect: URect,
    steps: u32,
    pivot: [u32; 2],
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

/// Pre-rotates each sprite newly under a bone that turns in steps, about the
/// joint: its transform's offset from the bone, where its middle is.
fn step(
    mut sprites: Query<(Entity, &ChildOf, &mut Sprite, &Transform), Without<Stepped>>,
    bones: Query<&Bone>,
    puppets: Query<&Puppet>,
    rigs: Res<Assets<Rig>>,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut steps: ResMut<Steps>,
    mut commands: Commands,
) {
    for (entity, parent, mut sprite, transform) in &mut sprites {
        let Some(turn) = bones.get(parent.parent()).ok().and_then(|bone| {
            let rig = rigs.get(&puppets.get(bone.puppet()).ok()?.0)?;
            rig.turn(bone.index())
        }) else {
            continue;
        };
        let rect = if let Some(atlas) = &sprite.texture_atlas {
            layouts
                .get(&atlas.layout)
                .and_then(|layout| layout.textures.get(atlas.index).copied())
        } else {
            sprite.rect.map(|rect| rect.as_urect()).or_else(|| {
                images
                    .get(&sprite.image)
                    .map(|image| URect::from_corners(UVec2::ZERO, image.size()))
            })
        };
        let Some(rect) = rect else {
            continue;
        };
        // The joint from the sprite's top left, down positive: its middle,
        // less where its middle is from the joint (up positive).
        let at = transform.translation.truncate();
        let pivot = rect.size().as_vec2() / 2.0 - Vec2::new(at.x, -at.y);
        let key = Key {
            image: sprite.image.id(),
            rect,
            steps: turn.steps,
            pivot: [pivot.x.to_bits(), pivot.y.to_bits()],
        };
        let (image, layout) = if let Some(made) = steps.0.get(&key) {
            made.clone()
        } else {
            let Some(rotsprite) = images
                .get(&sprite.image)
                .and_then(|image| RotSprite::new(image, rect, pivot))
            else {
                continue;
            };
            let made = (
                images.add(rotsprite.strip(turn.steps)),
                layouts.add(rotsprite.atlas(turn.steps)),
            );
            steps.0.insert(key, made.clone());
            made
        };
        sprite.image = image;
        sprite.rect = None;
        sprite.texture_atlas = Some(TextureAtlas { layout, index: 0 });
        commands.entity(entity).insert(Stepped {
            steps: turn.steps,
            z: transform.translation.z,
        });
    }
}

/// Shows each pre-rotated sprite's step for its bone's angle, upright in the
/// skeleton and pinned at the joint: the step turns, not the sprite.
fn show(
    mut sprites: Query<(&Stepped, &ChildOf, &mut Sprite, &mut Transform)>,
    bones: Query<&Placement, With<Bone>>,
) {
    for (stepped, parent, mut sprite, mut transform) in &mut sprites {
        let Ok(placement) = bones.get(parent.parent()) else {
            continue;
        };
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.index = crate::rig::Turn {
                steps: stepped.steps,
            }
            .step(placement.angle) as usize;
        }
        transform.translation = Vec3::new(0.0, 0.0, stepped.z);
        transform.rotation = Quat::from_rotation_z(-placement.angle);
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

    #[test]
    fn a_sprite_under_a_stepped_bone_shows_its_step_upright() {
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
            .add(rig(r#"(bones: {"arm": (turns: 4)})"#).unwrap());
        // Turned a quarter, counterclockwise.
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
                    width: 4,
                    height: 8,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[255, 0, 0, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::all(),
            ));
        // Its middle 4 down the joint: an arm hanging from its shoulder.
        let sprite = app
            .world_mut()
            .spawn((
                Sprite::from_image(image),
                Transform::from_xyz(0.0, -4.0, 0.0),
                ChildOf(arm),
            ))
            .id();
        app.update();
        let world = app.world();
        let sprite_on = world.get::<Sprite>(sprite).unwrap();
        assert_eq!(
            sprite_on.texture_atlas.as_ref().unwrap().index,
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
}
