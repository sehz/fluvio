mod config;

pub use config::MvOpt;

fn main() {

    use clap::Parser;
    
    use crate::MvOpt;
    
    fluvio_future::subscriber::init_tracer(None);

    let opt = MvOpt::parse();
    //  fluvio_spu::main_loop(opt);
}

