mod cli;

pub use config::*;
pub use cli::MvOpt;

mod config {

    #[derive(Debug, Eq, PartialEq, Clone, Default)]
    pub struct MvConfig {}

}