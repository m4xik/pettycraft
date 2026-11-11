// HEAD of the `perform_command` hook point: a player ran /pettycraft.
fn pettycraft_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &horizon_client::mod_api::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::pettycraft::perform_command(game, player, command, ran);
}
