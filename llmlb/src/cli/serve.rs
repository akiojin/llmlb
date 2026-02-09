//! serve サブコマンド
//!
//! ロードバランサーサーバーを起動します。

use clap::Args;

/// serve サブコマンドの引数
#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    /// Listen port
    #[arg(short, long, default_value = "32768", env = "LLMLB_PORT")]
    pub port: u16,

    /// Bind address
    #[arg(short = 'H', long, default_value = "0.0.0.0", env = "LLMLB_HOST")]
    pub host: String,

    /// Disable system tray (headless mode)
    #[arg(long, default_value_t = false)]
    pub no_tray: bool,
}
