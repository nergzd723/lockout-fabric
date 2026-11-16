//! Horizon with Lockout: what the launcher's generated executable does for a trait mod.
#[global_allocator]
static ALLOCATOR: horizon_threads::GameAllocator = horizon_threads::GAME_ALLOCATOR;

static MOD_API_LEVEL: [u8; horizon_mods::api_level::MARK.len()] = horizon_mods::api_level::MARK;

fn main() {
    std::hint::black_box(&MOD_API_LEVEL);
    let mods: horizon_client::mod_api::Mods = vec![mod_lockout::hooks()].into();
    horizon_client::mod_api::name_mods(&["lockout"]);
    horizon_client::entry::main_with(mods);
}
