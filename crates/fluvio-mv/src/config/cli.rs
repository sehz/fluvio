//!
//! # CLI for Streaming Processing Unit (SPU)
//!
//! Command line interface to provision SPU id and configure various
//! system parameters.
//!

use clap::Parser;



/// cli options
#[derive(Debug, Default, Parser)]
#[clap(name = "fluvio-mv", about = "Fluvio Materialized View")]
pub struct MvOpt {
   
}

impl MvOpt {
   
}
