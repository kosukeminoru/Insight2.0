use bevy::prelude::*;

use super::super::animation::{animation_helper, play};
use super::super::ggrs_rollback::network;
use super::super::players::info;

pub trait Power {
    fn my_movement(
        &self,
        p: &mut info::Player,
        player: &mut AnimationPlayer,
        animations: play::CharacterAnimations,
        transform: &mut Transform,
        commands: &mut Commands,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        animations_resource: &mut ResMut<Assets<AnimationClip>>,
        asset_server: &mut Res<AssetServer>,
    );

    fn effect(
        &self,
        p: &mut info::Player,
        player: &mut AnimationPlayer,
        animations: play::CharacterAnimations,
        transform: &mut Transform,
        commands: &mut Commands,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        animations_resource: &mut ResMut<Assets<AnimationClip>>,
        asset_server: &mut Res<AssetServer>,
    );
}

pub enum Tier {
    Basic,        // High # clicks, low impact_radius, low impact_extent.
    Intermediate, // High # clicks, high impact_radius, low impact_extent.
    Advanced,     // Low # clicks, low impact_radius, high impact_extent.
    God,          // Low # clicks, high impact_radius, high impact_extent.
}

pub enum Effect {
    Positive,
    Negative,
}

pub enum PowerType {
    HealthManip,    // Damage or increase health
    TranformManip,  // Change transform
    AnimationManip, // Change animation
    Object,         // Spawn object
}

pub enum Medium {
    Guesture,
    Weapon,
}

pub enum Affected {
    Me,
    Other,
}
