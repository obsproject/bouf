use env_logger::{Builder, Env};
use std::io::Write;

pub fn init_logger(level: &str) {
    let env = Env::default()
        .filter_or("BOUF_LOG_LEVEL", level)
        .write_style_or("BOUF_LOG_STYLE", "always");

    Builder::from_env(env)
        .format(|buf, record| {
            let timestamp = buf.timestamp();
            let style = buf.default_level_style(record.level());
            let level = record.level();
            let message = record.args();

            writeln!(buf, "[{timestamp}] {style}{level}{style:#}: {message}")
        })
        .init();
}
