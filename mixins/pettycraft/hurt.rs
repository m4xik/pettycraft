// HEAD of Game::hurt_by_player: who is about to hit which mob.
fn pettycraft_before_hurt(
    game: &mut crate::game::Game<'_>,
    index: usize,
    source: horizon_client::mobs::Damage,
) {
    crate::game::pettycraft::before_hurt(game, index, source);
}
