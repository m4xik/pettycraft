// Cancellable HEAD of Game::guest_command: a guest's /pettycraft runs as their turn.
fn pettycraft_guest_command(
    host: &mut crate::game::lan::LanHost,
    conn: u32,
    command: &str,
) -> core::ops::ControlFlow<()> {
    if crate::game::pettycraft::guest_command(host, conn, command) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
