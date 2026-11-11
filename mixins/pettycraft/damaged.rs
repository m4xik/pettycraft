// HEAD of Game::mob_damage_event: a player's hit lowered the mob's health.
fn pettycraft_damaged(game: &mut crate::game::Game<'_>, id: u32) {
    crate::game::pettycraft::damaged(game, id);
}
