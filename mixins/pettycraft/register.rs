// HEAD of the `register_commands` hook point: /pettycraft joins the command tree.
fn pettycraft_register_commands(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    crate::game::pettycraft::register_commands(commands);
}
