// HEAD of the `tick` hook point (Game::native_tick).
fn pettycraft_tick(game: &mut crate::game::Game<'_>) {
    crate::game::pettycraft::tick(game);
}
