#[macro_use]
extern crate delog;

local_delog!();

pub fn g() {
    info!("global info log from lib-b");
    warn!("global info log from lib-b");

    local_log!(delog::Level::Info, "log level info from lib_b::g");
    local_log!(target: "!", delog::Level::Info, "log level info from lib_b::g");
    local_log!(target: "!", delog::Level::Warn, "log level warn from lib_b::g");

    local_info!("local info from lib_b::g");
    local_warn!("local warn from lib_b::g");
    local_info!(target: "!", "immediate local info from lib_b::g");
    local_warn!(target: "!", "immediate local warn from lib_b::g");
}
