// Cancellable HEAD of Game::run_command: /pettycraft on the authoritative game.
fn pettycraft_command(
    game: &mut crate::game::Game<'_>,
    command: &str,
) -> core::ops::ControlFlow<bool> {
    match crate::game::pettycraft::command(game, command) {
        Some(ran) => core::ops::ControlFlow::Break(ran),
        None => core::ops::ControlFlow::Continue(()),
    }
}
