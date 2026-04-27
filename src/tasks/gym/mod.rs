// Gym tracker background tasks
pub mod weekly_check;
pub mod loa_check;

pub use loa_check::resolve_loa_vote;

pub fn start_gym_weekly_check(http: std::sync::Arc<poise::serenity_prelude::Http>, data: std::sync::Arc<crate::Data>) {
    weekly_check::start_weekly_check_task(http.clone(), data.clone());
    loa_check::start_loa_check_task(http, data);
}
