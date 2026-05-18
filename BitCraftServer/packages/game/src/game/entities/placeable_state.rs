use std::time::Duration;

use spacetimedb::rand::Rng;
use spacetimedb::{ReducerContext, Table};

use crate::messages::components::{GrowthState, LocationState, PlaceableState};
use crate::messages::static_data::*;
use crate::{
    game::{autogen::_delete_entity::delete_entity, coordinates::*, game_state},
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
            let duration = if growth.time.len() <= 1 {
                growth.time.first().copied().unwrap_or(0.0)
            } else {
                ctx.rng().gen_range(growth.time[0]..=growth.time[1])
            };

            ctx.db
                .growth_state()
                .try_insert(GrowthState {
                    entity_id,
                    end_timestamp: ctx.timestamp + Duration::from_secs_f32(duration),
                    growth_recipe_id: growth.id,
                })
                .unwrap();
        }
    }
}
