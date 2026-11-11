// HEAD of the `world_lifecycle` hook point: save when the world closes.
fn pettycraft_lifecycle(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::WorldPhase,
) {
    crate::game::pettycraft::lifecycle(game, phase);
}
