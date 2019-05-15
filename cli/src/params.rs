use cli::{AugmentClap, GetLogFilter};
use structopt::{clap::App, StructOpt};
use vendor::RunStrategy;

#[derive(Debug, StructOpt, Clone, Default)]
pub struct VendorCmd {
    /// Enable listener mode
    #[structopt(long = "listener")]
    pub listener: bool,
    /// Enable sender mode
    #[structopt(long = "sender")]
    pub sender: bool,
}

impl GetLogFilter for VendorCmd {
    fn get_log_filter(&self) -> Option<String> {
        None
    }
}

impl AugmentClap for VendorCmd {
    fn augment_clap<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
        app
    }
}

impl Into<RunStrategy> for VendorCmd {
    fn into(self) -> RunStrategy {
        RunStrategy {
            listener: self.listener,
            sender: self.sender,
        }
    }
}
