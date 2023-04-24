use env_logger::{Builder, Env};
use std::io::Write;

pub fn init_logger(level: &str) {
    let env = Env::default()
        .filter_or("BOUF_LOG_LEVEL", level)
        .write_style_or("BOUF_LOG_STYLE", "always");

    Builder::from_env(env)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] {}: {}",
                buf.timestamp(),
                buf.default_styled_level(record.level()),
                record.args()
            )
        })
        .init();
}
