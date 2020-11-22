#[macro_use]
extern crate delog;

local_delog!();

pub fn f() {
    info!("global info log from lib_a");
    warn!("global info log from lib_a");

    local_log!(delog::Level::Info, "log level info from lib_a::f");
    local_log!(target: "!", delog::Level::Info, "log level info from lib_a::f");
    local_log!(target: "!", delog::Level::Warn, "log level warn from lib_a::f");

    local_info!("local info from lib_a::f");
    local_warn!("local warn from lib_a::f");
    local_info!(target: "!", "immediate local info from lib_a::f");
    local_warn!(target: "!", "immediate local warn from lib_a::f");
}
