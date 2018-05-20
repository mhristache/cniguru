extern crate docopt;
extern crate env_logger;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate failure;
extern crate kubeclient;
extern crate url;

// modules
mod error;
mod k8s;

use docopt::Docopt;
use failure::Error;
use std::io::Write;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

fn version() -> String {
    format!("yacht {} ({})", semver(), commit_date())
}

const USAGE: &'static str = "
Usage: cniguru pod <id> [-n <namespace>]
       cniguru dc <id>
       cniguru [-h] [--version]

Options:
    -h, --help         Show this message.
    --version          Show the version
    -n                 Specify a kubernetes namespace

Main commands:
    pod                The name of a kubernetes pod
    dc                 The name or id of a docker container
";

#[derive(Debug, Deserialize)]
struct Args {
    cmd_pod: bool,
    cmd_dc: bool,
    arg_id: String,
    arg_namespace: Option<String>,
    flag_version: bool,
}

fn write_err_and_exit(e: Error, code: i32) -> ! {
    debug!("error details: {:?}", e);
    if let Err(_) = write!(std::io::stderr(), "Error: {}\n", e) {
        panic!("could not write to stderr");
    }
    std::process::exit(code);
}

fn main() {
    env_logger::init();

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    debug!("program args: {:?}", args);

    if args.flag_version {
        println!("{}", version());
        return;
    }

    if args.cmd_pod {
        let pod = k8s::Pod::new(&args.arg_id, args.arg_namespace.as_ref().map(|x| &x[..]));
        let containers = pod.containers().unwrap();
        println!("{:#?}", containers);
    } else if args.cmd_dc {
        unimplemented!();
    } else {
        println!("Not enough arguments.\n{}", &USAGE);
    }
}

trait LinuxNamespace {
    fn pid() -> u32;
}
