use spacetimedb::rand::Rng;
use spacetimedb::{ReducerContext, Table};

use crate::messages::components::{GrowthState, LocationState, PlaceableState};
use crate::messages::static_data::*;
use crate::{
    game::{
        autogen::_delete_entity::delete_entity, coordinates::*, dimensions, game_state, game_state::game_state_filters,
        terrain_chunk::TerrainChunkCache,
    },
    growth_state, health_state, location_state, placeable_growth_desc, placeable_state, unwrap_or_err, HealthState,
};

impl PlaceableState {
    pub fn get_at_location(ctx: &ReducerContext, coordinates: &SmallHexTile) -> Option<PlaceableState> {
        LocationState::select_all(ctx, coordinates)
            .filter_map(|ls| ctx.db.placeable_state().entity_id().find(&ls.entity_id))
            .next()
    }

    pub fn distance_to(&self, ctx: &ReducerContext, coordinates: SmallHexTile) -> i32 {
        if let Some(location) = ctx.db.location_state().entity_id().find(&self.entity_id) {
            location.coordinates().distance_to(coordinates)
        } else {
            i32::MAX
        }
    }

    pub fn spawn(
        ctx: &ReducerContext,
        placeable_id: i32,
        owner_entity_id: u64,
        coordinates: SmallHexTile,
        direction_index: i32,
    ) -> Result<(), String> {
        let placeable_desc = unwrap_or_err!(ctx.db.placeable_desc().id().find(&placeable_id), "Unknown placeable");
        let entity_id = game_state::create_entity(ctx);

        ctx.db.placeable_state().try_insert(PlaceableState {
            entity_id,
            owner_entity_id,
            placeable_id,
            direction_index,
        })?;

        ctx.db.health_state().try_insert(HealthState {
            entity_id,
            last_health_decrease_timestamp: ctx.timestamp,
            health: placeable_desc.max_health as f32,
            died_timestamp: 0,
        })?;

        game_state::insert_location(ctx, entity_id, coordinates.to_offset_coordinates());
        Self::add_growth_state(ctx, entity_id, placeable_id);

        Ok(())
    }

    pub fn despawn(&self, ctx: &ReducerContext) {
        delete_entity(ctx, self.entity_id);
    }

    pub fn produce_offspawn(
        ctx: &ReducerContext,
        owner_entity_id: u64,
        coordinates: SmallHexTile,
        direction_index: i32,
        spawned_placeable_id: i32,
        spawn_chance: f32,
    ) -> Result<(), String> {
        if spawned_placeable_id == 0 || spawn_chance <= 0.0 {
            return Ok(());
        }

        let should_spawn = if spawn_chance >= 1.0 {
            true
        } else {
            ctx.rng().gen_range(0.0..=1.0) <= spawn_chance
        };

        if should_spawn {
            Self::spawn(ctx, spawned_placeable_id, owner_entity_id, coordinates, direction_index)?;
        }

        Ok(())
    }

    pub fn spawn_in_radius_band(
        ctx: &ReducerContext,
        placeable_id: i32,
        owner_entity_id: u64,
        center: SmallHexTile,
        direction_index: i32,
        min_radius: i32,
        max_radius: i32,
    ) -> Result<bool, String> {
        if placeable_id == 0 {
            return Ok(false);
        }

        let mut terrain_cache = TerrainChunkCache::empty();
        let min_radius = min_radius.max(0);
        let max_radius = max_radius.max(0);

        let candidates = if min_radius == 0 && max_radius == 0 {
            vec![center]
        } else {
            SmallHexTile::shuffled_coordinates_between_radius(center, min_radius, max_radius, &mut ctx.rng())
        };

        for coordinates in candidates {
            if !Self::is_valid_spawn_tile(ctx, &mut terrain_cache, coordinates) {
                continue;
            }

            Self::spawn(ctx, placeable_id, owner_entity_id, coordinates, direction_index)?;
            return Ok(true);
        }

        Ok(false)
    }

    pub fn pick_growth_outcome(ctx: &ReducerContext, outcomes: &[PlaceableGrowthOutcome]) -> i32 {
        if outcomes.is_empty() {
            return 0;
        }

        let mut sum = 0.0;
        for outcome in outcomes {
            sum += outcome.probability;
        }

        if sum <= 0.0 {
            return 0;
        }

        let mut rnd = ctx.rng().gen_range(0.0..=sum);
        for outcome in outcomes {
            rnd -= outcome.probability;
            if rnd <= 0.0 {
                return outcome.placeable_id;
            }
        }

        0
    }

    fn add_growth_state(ctx: &ReducerContext, entity_id: u64, placeable_id: i32) {
        if let Some(growth) = ctx.db.placeable_growth_desc().placeable_id().find(&placeable_id) {
            let _ = ctx.db.growth_state().try_insert(GrowthState::new_from_placeable(ctx, entity_id, growth));
        }
    }

    fn is_valid_spawn_tile(ctx: &ReducerContext, terrain_cache: &mut TerrainChunkCache, coordinates: SmallHexTile) -> bool {
        if game_state_filters::has_hitbox_footprint(ctx, coordinates) {
            return false;
        }

        if LocationState::select_all(ctx, &coordinates)
            .filter_map(|location| ctx.db.placeable_state().entity_id().find(&location.entity_id))
            .next()
            .is_some()
        {
            return false;
        }

        if coordinates.dimension != dimensions::OVERWORLD && !game_state_filters::is_interior_tile_walkable(ctx, coordinates) {
            return false;
        }

        terrain_cache.get_terrain_cell(ctx, &coordinates.parent_large_tile()).is_some()
    }
}
