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
extern crate regex;
extern crate url;
#[macro_use]
extern crate lazy_static;

// modules
mod error;
mod k8s;
mod tests;

use docopt::Docopt;
use failure::{Error, ResultExt, Fail};
use regex::Regex;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::Command;
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

fn main() {
    env_logger::init();

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    debug!("program args: {:?}", args);

    if let Err(e) = try_main(args) {
        let mut fail: &Fail = e.cause();
        let mut f = std::io::stderr();
        write!(std::io::stderr(), "error: {}\n", fail).expect("could not write to stderr");
        while let Some(cause) = fail.cause() {
            write!(f, "caused by: {}\n", cause).expect("could not write to stderr");
            fail = cause;
        }
        if std::env::var("RUST_BACKTRACE").is_ok() {
            write!(f, "{}\n", e.backtrace()).expect("could not write to stderr");
        }
        std::process::exit(1);
    }
}

fn try_main(args: Args) -> Result<(), Error> {
    if args.flag_version {
        println!("{}", version());
        return Ok(());
    }

    if args.cmd_pod {
        let pod = k8s::Pod::new(&args.arg_id, args.arg_namespace.as_ref().map(|x| &x[..]));
        let err_ctx = format!(
            "failed to get info about containers in pod '{}' on namespace '{}'",
            pod.name, pod.namespace
        );
        let containers = pod.containers().context(err_ctx)?;
        for container in containers {
            gen_output_for_container(container)?;
        }
    } else if args.cmd_dc {
        let container = Container {
            id: args.arg_id,
            node_name: None,
            runtime: ContainerRuntime::Docker,
        };
        gen_output_for_container(container)?;
    } else {
        println!("Not enough arguments.\n{}", &USAGE);
        std::process::exit(1);
    }
    Ok(())
}

// generate the output for the given container
fn gen_output_for_container(container: Container) -> Result<(), Error> {
    let ctx1 = format!(
        "failed to find the namespace for container id {}",
        &container.id
    );
    let ctx2 = format!(
        "failed to find the host interfaces for container id {}",
        &container.id
    );
    let intfs = container.netns().context(ctx1)?.interfaces().context(ctx2)?;
    println!("{:?}", intfs);
    Ok(())
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Intf {
    name: String,
    mtu: u16,
    mac_address: String,
    bridge: Option<String>,
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
                let cmd = format!("docker inspect {} --format '{{{{.State.Pid}}}}'", &self.id);
                let output = run_host_cmd(&cmd)?;
                let pid: u32 = output.trim_matches('\'').parse()?;
                Ok(pid)
            }
        }
    }

    // get the linux netns for this container
    fn netns(&self) -> Result<Netns, Error> {
        let pid = self.pid()?;
        Ok(Netns::new(pid))
    }
}

struct Netns {
    pid: u32,
    rmdir_needed: bool,
    rmlink_needed: bool,
    dst_path: PathBuf,
    src_path: PathBuf,
}

impl Netns {
    fn new(pid: u32) -> Self {
        Self {
            pid: pid,
            rmdir_needed: false,
            rmlink_needed: false,
            dst_path: PathBuf::from(format!("/var/run/netns/ns-{}", pid)),
            src_path: PathBuf::from(format!("/proc/{}/ns/net", pid)),
        }
    }

    fn interfaces(&mut self) -> Result<Vec<Intf>, Error> {
        // a link from /proc/<pid>/ns/net to /var/run/netns/<some id> must exist
        // so that `ip netns` commands can be used
        let parent_dir = self.dst_path.parent().unwrap();
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)?;
            self.rmdir_needed = true;
        }

        if !self.dst_path.exists() {
            symlink(self.src_path.as_path(), self.dst_path.as_path())?;
            self.rmlink_needed = true;
        }

        debug!("trying to find the namespace id for pid {}", self.pid);
        let cmd = format!("ip netns identify {}", self.pid);
        let output = run_host_cmd(&cmd)?;

        debug!("trying to find link-netnsid for ns {}", &output);
        let cmd = format!("ip netns list | grep {}", &output);
        let output = run_host_cmd(&cmd)?;

        // the expected format of the output is something like `ns-56316 (id: 6)`
        debug!("extracting the link-netnsid from {}", &output);
        lazy_static! {
            static ref RE1: Regex = Regex::new(r"\(id: (\d+)\)").unwrap();
        }
        let id: u32 = match RE1.captures(&output).and_then(|m| m.get(1)) {
            Some(v) => v.as_str().parse()?,
            None => return Err(error::DataExtractionError::OutputParsingError(cmd))?,
        };

        debug!("fetching ip link printout");
        let cmd = "ip link show";
        let output = run_host_cmd(cmd)?;

        parse_ip_link_printout(&output, id)
    }
}

fn parse_ip_link_printout(printout: &str, id: u32) -> Result<Vec<Intf>, Error> {
    debug!("parsing ip link printout to check for link-netnsid {}", &id);
    let mut res = vec![];
    let s = concat!(
        r"\d+:\s+(?P<name>\w+)@\w+:",
        r".*\s+mtu\s+(?P<mtu>\d+)\s+",
        r"(?:.*\s+master\s+(?P<br>\w+)\s+)?",
        r".*\s+link/ether\s+(?P<mac>(\w|:)+)\s+",
        r".*link-netnsid");
    let s = format!("{} {}", s, id);
    let re = Regex::new(&s).unwrap();
    let err = error::IntfMatchError(id);
    for m in re.captures_iter(printout) {
        let intf = Intf {
            name: m.name("name").ok_or(err)?.as_str().to_string(),
            mtu: m.name("mtu").ok_or(err)?.as_str().parse()?,
            bridge: m.name("br").map(|v| v.as_str().to_string()),
            mac_address: m.name("mac").ok_or(err)?.as_str().to_string(),
        };
        res.push(intf);
    }
    if res.len() == 0 {
        Err(err)?
    } else {
        Ok(res)
    }
}

impl Drop for Netns {
    /// Drop is used to remove files created by self.interfaces() logic
    fn drop(&mut self) {
        if self.rmlink_needed {
            match fs::remove_file(self.dst_path.as_path()) {
                Ok(_) => debug!("symlink removed successfully"),
                Err(e) => debug!("failed to remove the symlink: {}", e),
            }
        }
        if self.rmdir_needed {
            let parent_dir = self.dst_path.parent().unwrap();
            match fs::remove_dir(parent_dir) {
                Ok(_) => debug!("parent dir removed successfully"),
                Err(e) => debug!("failed to remove the parent dir: {}", e),
            }
        }
    }
}

/// Run a command on the host and return the trimmed output.
/// Raise an error if the command did not run successfully
fn run_host_cmd(cmd: &str) -> Result<String, Error> {
    let cmd_parts: Vec<&str> = cmd.split(' ').collect();

    // the first element of the vec is the name of the program to run and the rest are arguments
    let (prog, args) = match cmd_parts.as_slice().split_first() {
        Some(v) => v,
        None => Err(error::HostCmdError::CmdInvalid(cmd.to_string()))?,
    };
    debug!("running '{}' with args {:?}", prog, args);

    let output = Command::new(prog).args(args).output()?;

    let se = std::str::from_utf8(&output.stderr[..])?.trim();
    let so = std::str::from_utf8(&output.stdout[..])?.trim();
    trace!("\nstdout: {}\nstderr: {}", so, se);

    if output.status.success() {
        Ok(so.to_string())
    } else {
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or("N/A".to_string());
        Err(error::HostCmdError::CmdFailed {
            cmd: cmd.to_string(),
            code,
            stderr: se.to_string(),
        })?
    }
}

