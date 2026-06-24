use std::time::Duration;

use spacetimedb::{rand::Rng, ReducerContext, ScheduleAt, Table};

use crate::{
    inter_module::system_chat_broadcast::{sytem_chat_broadcast_timer, SystemChatBroadcastTimer},
    messages::{
        components::{location_state, GrowthState},
        generic::world_region_state,
        static_data::{PlaceableGrowthDesc, ResourceGrowthRecipeDesc},
    },
    unwrap_or_return,
};

impl GrowthState {
    pub fn new(ctx: &ReducerContext, entity_id: u64, resource_growth_recipe_desc: ResourceGrowthRecipeDesc) -> GrowthState {
        let end_timestamp = ctx.timestamp + Duration::from_secs_f32(get_duration(ctx, resource_growth_recipe_desc.time.clone()));
        broadcast_growth(ctx, entity_id, &resource_growth_recipe_desc, end_timestamp);

        GrowthState {
            entity_id,
            end_timestamp,
            growth_recipe_id: resource_growth_recipe_desc.id,
        }
    }

    pub fn new_from_placeable(ctx: &ReducerContext, entity_id: u64, placeable_growth_desc: PlaceableGrowthDesc) -> GrowthState {
        GrowthState {
            entity_id,
            end_timestamp: ctx.timestamp + Duration::from_secs_f32(get_duration(ctx, placeable_growth_desc.time)),
            growth_recipe_id: placeable_growth_desc.id,
        }
    }
}

fn get_duration(ctx: &ReducerContext, time: Vec<f32>) -> f32 {
    if time.len() <= 1 {
        return time.first().copied().unwrap_or(0.0);
    }
    ctx.rng().gen_range(time[0]..=time[1])
}

fn broadcast_growth(
    ctx: &ReducerContext,
    entity_id: u64,
    resource_growth_recipe_desc: &ResourceGrowthRecipeDesc,
    end_timestamp: spacetimedb::Timestamp,
) {
    const INACTIVE_HEXITE_SEALED_CHEST_GROWTH_ID: i32 = 1633012503;
    if resource_growth_recipe_desc.id != INACTIVE_HEXITE_SEALED_CHEST_GROWTH_ID {
        return;
    }

    let location = unwrap_or_return!(ctx.db.location_state().entity_id().find(entity_id), "Unknown location")
        .coordinates()
        .parent_large_tile()
        .to_offset_coordinates();
    let region = unwrap_or_return!(ctx.db.world_region_state().id().find(0), "Unknown region");

    ctx.db.sytem_chat_broadcast_timer().insert(SystemChatBroadcastTimer {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Time(end_timestamp - Duration::from_hours(1)),
        message: format!(
            "The (res={{0}}) in Region {{1}} at (coord={{2}},{{3}}) is preparing to unlock in 1 hour.|~{}|~{}|~{}|~{}",
            resource_growth_recipe_desc.resource_id, region.region_index, location.z, location.x
        ),
    });

    ctx.db.sytem_chat_broadcast_timer().insert(SystemChatBroadcastTimer {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Time(end_timestamp - Duration::from_mins(15)),
        message: format!(
            "The (res={{0}}) in Region {{1}} at (coord={{2}},{{3}}) is preparing to unlock in 15 minutes.|~{}|~{}|~{}|~{}",
            resource_growth_recipe_desc.resource_id, region.region_index, location.z, location.x
        ),
    });

    ctx.db.sytem_chat_broadcast_timer().insert(SystemChatBroadcastTimer {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Time(end_timestamp - Duration::from_mins(5)),
        message: format!(
            "The (res={{0}}) in Region {{1}} at (coord={{2}},{{3}}) is preparing to unlock in 5 minutes.|~{}|~{}|~{}|~{}",
            resource_growth_recipe_desc.resource_id, region.region_index, location.z, location.x
        ),
    });
}
