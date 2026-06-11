use bitcraft_macro::shared_table_reducer;
use spacetimedb::{ReducerContext, Table};
use crate::inter_module::_autogen::InterModuleTableUpdatesV2;

use crate::{
    game::handlers::{authentication::has_role, empires::*, player::sign_out::sign_out_internal, queue::player_queue},
    messages::{
        authentication::{Role, ServerIdentity},
        inter_module::{
            inter_module_message_counter, inter_module_message_errors, inter_module_message_v4, InterModuleMessageCounter,
            InterModuleMessageErrors, InterModuleMessageV4, MessageContentsV4,
        },
    },
};

use super::*;

//Called on destination module
#[spacetimedb::reducer]
#[shared_table_reducer]
pub fn process_inter_module_message(ctx: &ReducerContext, sender: u8, message: InterModuleMessageV4) -> Result<(), String> {
    validate_relay_identity(ctx)?;

    if let Some(mut counter) = ctx.db.inter_module_message_counter().module_id().find(&sender) {
        if counter.last_processed_message_id >= message.id {
            //Message was already processed
            spacetimedb::log::warn!("Inter-module message {} was already processed", message.id);
            if let Some(r) = ctx.db.inter_module_message_errors().id().filter((sender, message.id)).next() {
                return Err(r.error);
            }
            return Ok(());
        }
        counter.last_processed_message_id = message.id;
        ctx.db.inter_module_message_counter().module_id().update(counter);
    } else {
        ctx.db.inter_module_message_counter().insert(InterModuleMessageCounter {
            module_id: sender,
            last_processed_message_id: message.id,
        });
    }

    let r = match message.contents {
        MessageContentsV4::TableUpdate(u) => {
            apply_inter_module_table_update(ctx, u);
            Ok(())
        }

        MessageContentsV4::UserUpdateRegionRequest(_) => panic!("Region module should never receive UserUpdateRegionRequest message"),
        MessageContentsV4::ClaimCreateEmpireSettlementState(_) => {
            panic!("Region module should never receive ClaimCreateEmpireSettlementState message")
        }
        MessageContentsV4::OnClaimMembersChanged(_) => panic!("Region module should never receive OnClaimMembersChanged message"),
        MessageContentsV4::EmpireCreateBuilding(_) => panic!("Region module should never receive EmpireCreateBuilding message"),
        MessageContentsV4::GlobalDeleteEmpireBuilding(_) => panic!("Region module should never receive GlobalDeleteEmpireBuilding message"),
        MessageContentsV4::DeleteEmpire(_) => panic!("Region module should never receive DeleteEmpire message"),
        MessageContentsV4::EmpireClaimJoin(_) => panic!("Region module should never receive EmpireClaimJoin message"),
        MessageContentsV4::EmpireResupplyNode(_) => panic!("Region module should never receive EmpireResupplyNode message"),
        MessageContentsV4::EmpireDonateItem(_) => panic!("Region module should never receive EmpireDonateItem message"),
        MessageContentsV4::EmpireCreate(_) => panic!("Region module should never receive EmpireCreate message"),
        MessageContentsV4::EmpireCollectHexiteCapsule(_) => panic!("Region module should never receive EmpireCollectHexiteCapsule message"),
        MessageContentsV4::EmpireStartSiege(_) => panic!("Region module should never receive EmpireStartSiege message"),
        MessageContentsV4::EmpireSiegeAddSupplies(_) => panic!("Region module should never receive EmpireSiegeAddSupplies message"),
        MessageContentsV4::OnRegionPlayerCreated(_) => panic!("Region module should never receive OnRegionPlayerCreated message"),
        MessageContentsV4::EmpireQueueSupplies(_) => panic!("Region module should never receive EmpireQueueSupplies message"),
        MessageContentsV4::EmpireAddCurrency(_) => panic!("Region module should never receive EmpireAddCurrency message"),
        MessageContentsV4::ClaimSetName(_) => panic!("Region module should never receive ClaimSetName message"),
        MessageContentsV4::NpcPlaceWatchtowers(_) => panic!("Region module should never receive NpcPlaceWatchtowers message"),
        MessageContentsV4::EmpireWithdrawItem(_) => panic!("Region module should never receive EmpireWithdrawItem message"),

        MessageContentsV4::TransferPlayerRequest(r) => transfer_player::process_message_on_destination(ctx, sender, r),
        MessageContentsV4::TransferPlayerHousingRequest(r) => transfer_player_housing::process_message_on_destination(ctx, r),
        MessageContentsV4::PlayerCreateRequest(r) => player_create::process_message_on_destination(ctx, r),
        MessageContentsV4::OnPlayerNameSetRequest(r) => on_player_name_set::process_message_on_destination(ctx, r),
        MessageContentsV4::OnEmpireBuildingDeleted(r) => on_empire_building_deleted::process_message_on_destination(ctx, r),
        MessageContentsV4::OnPlayerJoinedEmpire(r) => on_player_joined_empire::process_message_on_destination(ctx, r),
        MessageContentsV4::OnPlayerLeftEmpire(r) => on_player_left_empire::process_message_on_destination(ctx, r),
        MessageContentsV4::RegionDestroySiegeEngine(r) => region_destroy_siege_engine::process_message_on_destination(ctx, r),
        MessageContentsV4::EmpireUpdateEmperorCrown(r) => empire_update_emperor_crown::process_message_on_destination(ctx, r),
        MessageContentsV4::EmpireRemoveCrown(r) => empire_remove_crown::process_message_on_destination(ctx, r),
        MessageContentsV4::SignPlayerOut(r) => {
            sign_out_internal(ctx, r.player_identity, false);
            Ok(())
        }
        MessageContentsV4::AdminBroadcastMessage(_) => panic!("Region module should never receive AdminBroadcastMessage message"),
        MessageContentsV4::PlayerSkipQueue(r) => player_skip_queue::process_message_on_destination(ctx, r),
        MessageContentsV4::GrantHubItem(r) => grant_hub_item::process_message_on_destination(ctx, r),
        MessageContentsV4::RecoverDeployable(r) => recover_deployable::process_message_on_destination(ctx, sender, r),
        MessageContentsV4::OnDeployableRecovered(r) => on_deployable_recovered::process_message_on_destination(ctx, r),
        MessageContentsV4::ReplaceIdentity(r) => replace_identity::process_message_on_destination(ctx, r),
        MessageContentsV4::RestoreSkills(r) => restore_skills::process_message_on_destination(ctx, r),
    };

    if let Err(error) = r.clone() {
        spacetimedb::volatile_nonatomic_schedule_immediate!(save_inter_module_message_error(sender, message.id, error));
    }

    return r;
}

#[spacetimedb::reducer()]
fn save_inter_module_message_error(ctx: &ReducerContext, sender: u8, message_id: u64, error: String) {
    if let Err(_) = ServerIdentity::validate_server_only(ctx) {
        return;
    }
    ctx.db.inter_module_message_errors().insert(InterModuleMessageErrors {
        sender_module_id: sender,
        message_id: message_id,
        error: error,
    });
}

//Called on sender module
#[spacetimedb::reducer]
#[shared_table_reducer]
pub fn on_inter_module_message_processed(ctx: &ReducerContext, id: u64, error: Option<String>) -> Result<(), String> {
    validate_relay_identity(ctx)?;

    if let Some(err) = &error {
        spacetimedb::log::error!("Inter-module reducer {id} returned error: {err}");
    }

    let message = match ctx.db.inter_module_message_v4().id().find(id) {
        Some(m) => m,
        None => {
            return Err(format!(
                "No inter_module_message for id {{0}}. Is there more than one relay running?|~{id}",
            ))
        }
    };
    match message.contents {
        MessageContentsV4::TransferPlayerRequest(r) => transfer_player::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::TransferPlayerHousingRequest(r) => transfer_player_housing::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::EmpireResupplyNode(r) => empire_resupply_node::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::EmpireDonateItem(r) => empire_donate_item::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::EmpireCreate(r) => empire_create::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::EmpireCollectHexiteCapsule(r) => {
            empire_collect_hexite_capsule::handle_destination_result_on_sender(ctx, r, error)
        }
        MessageContentsV4::EmpireStartSiege(r) => empire_start_siege::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::EmpireSiegeAddSupplies(r) => empire_siege_add_supplies::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::EmpireCreateBuilding(r) => empire_create_building::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::EmpireClaimJoin(r) => empire_claim_join::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::EmpireQueueSupplies(r) => empire_queue_supplies::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::RecoverDeployable(r) => recover_deployable::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::ClaimSetName(r) => claim_set_name::handle_destination_result_on_sender(ctx, r, error),
        MessageContentsV4::EmpireWithdrawItem(r) => empire_withdraw_item::handle_destination_result_on_sender(ctx, r, error),
        _ => {}
    }

    ctx.db.inter_module_message_v4().id().delete(id);
    return Ok(());
}

fn validate_relay_identity(ctx: &ReducerContext) -> Result<(), String> {
    if !has_role(ctx, &ctx.sender, Role::Admin) {
        return Err("Unauthorized".into());
    }
    return Ok(());
}

fn apply_inter_module_table_update(ctx: &ReducerContext, inter_module_table_updates: InterModuleTableUpdatesV2) {
    let is_region_sign_in_parameters = inter_module_table_updates.region_sign_in_parameters.is_some();

    inter_module_table_updates.apply_updates(ctx);

    if is_region_sign_in_parameters {
        player_queue::process_queue(ctx);
    }
}
