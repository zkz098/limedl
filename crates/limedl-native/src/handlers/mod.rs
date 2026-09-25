pub mod inspector;
pub mod labs;
pub mod new_task;
pub mod settings;
pub mod setup_wizard;
pub mod task;
pub mod updater;
pub mod window;

use crate::context::AppContext;

pub fn register_all(ctx: &AppContext) {
    task::register(ctx);
    new_task::register(ctx);
    inspector::register(ctx);
    settings::register(ctx);
    setup_wizard::register(ctx);
    labs::register(ctx);
    updater::register(ctx);
    window::register(ctx);
}
