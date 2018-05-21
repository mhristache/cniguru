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
use std::process::Command;

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
        for container in containers {
            println!("{:?}", container.pid());
        }
    } else if args.cmd_dc {
        let container = Container {
            id: args.arg_id,
            node_name: None,
            runtime: ContainerRuntime::Docker,
        };
        println!("{:?}", container.pid());
    } else {
        println!("Not enough arguments.\n{}", &USAGE);
    }
}

#[derive(Debug)]
pub enum ContainerRuntime {
    Docker,
}

#[derive(Debug)]
pub struct Container {
    pub id: String,
    pub node_name: Option<String>,
    pub runtime: ContainerRuntime,
}

impl Container {
    fn pid(&self) -> Result<u32, Error> {
        match self.runtime {
            ContainerRuntime::Docker => {
                // fetch the PID using docker CLI
                // a docker client is not currently used as it's hard to find a lightweight
                // and good enough one in the rust ecosystem
                debug!("trying to find the pid for docker container {}", &self.id);
                //let cmd = format!("/usr/bin/docker inspect {} --format '{{.State.Pid}}'", &self.id);
                let output = Command::new("docker")
                    .arg("inspect")
                    .arg(&self.id)
                    .arg("--format")
                    .arg("{{.State.Pid}}")
                    .output()?;

                if output.status.success() {
                    let so = std::str::from_utf8(&output.stdout[..])?.trim();
                    debug!("cmd stdout: {}", so);
                    let pid: u32 = so.parse()?;
                    Ok(pid)
                } else {
                    let se = std::str::from_utf8(&output.stderr[..])?;
                    let details = format!(
                        "docker inspect failed for container {} with code {:?}, error: {}",
                        &self.id,
                        output.status.code(),
                        se
                    );
                    Err(error::DockerError::DockerCommandError { details })?
                }
            }
        }
    }
}
