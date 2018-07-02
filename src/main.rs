extern crate docopt;
extern crate env_logger;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate failure;
extern crate kubeclient;
extern crate regex;
extern crate url;
#[macro_use]
extern crate lazy_static;
extern crate tabwriter;

// modules
mod error;
mod k8s;
#[cfg(test)]
mod tests;

use docopt::Docopt;
use failure::{Error, Fail, ResultExt};
use regex::Regex;
use std::io::Write;
use std::process::Command;
use tabwriter::TabWriter;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

fn version() -> String {
    format!("cniguru {} ({})", semver(), commit_date())
}

const USAGE: &'static str = "
Usage: cniguru pod <id> [-n <namespace> ] [-o <output>]
       cniguru dc <id> [-o <output> ]
       cniguru [-h] [--version]

Options:
    -h, --help         Show this message.
    --version          Show the version
    -n <namespace>     Specify a kubernetes namespace
    -o <output>        Specify a different way to format the output, e.g. json

Main commands:
    pod                The name of a kubernetes pod
    dc                 The name or id of a docker container
";

#[derive(Debug, Deserialize)]
struct Args {
    cmd_pod: bool,
    cmd_dc: bool,
    arg_id: String,
    flag_n: Option<String>,
    flag_o: Option<OutputFormat>,
    flag_version: bool,
}

#[derive(Debug, Deserialize)]
enum OutputFormat {
    JSON,
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

    match try_main(&args) {
        Ok(v) => match args.flag_o {
            Some(OutputFormat::JSON) => println!(
                "{}",
                serde_json::to_string(&v).expect("failed to serialize the output to json")
            ),
            None => pretty_print_output_and_exit(v),
        },
        Err(e) => match args.flag_o {
            Some(OutputFormat::JSON) => print_err_as_json_and_exit(e),
            None => pretty_print_err_and_exit(e),
        },
    }
}

/// Wrapper on top of `main()` to be able to use `?` for error handling
fn try_main(args: &Args) -> Result<Vec<Output>, Error> {
    let mut output_vec = vec![];

    if args.cmd_pod {
        let pod = k8s::Pod::new(&args.arg_id, args.flag_n.as_ref().map(|x| &x[..]));
        let err_ctx = format!(
            "failed to get info about containers in pod '{}' on namespace '{}'",
            pod.name, pod.namespace
        );
        let containers = pod.containers().context(err_ctx)?;
        for container in containers {
            let output = gen_output_for_container(container)?;
            output_vec.push(output);
        }
    } else if args.cmd_dc {
        let container = Container::new(args.arg_id.clone(), ContainerRuntime::Docker)?;
        let output = gen_output_for_container(container)?;
        output_vec.push(output);
    } else {
        println!("Not enough arguments.\n{}", &USAGE);
        std::process::exit(1);
    }
    Ok(output_vec)
}

/// Generate the `Output` struct for the given container
fn gen_output_for_container(container: Container) -> Result<Output, Error> {
    let ctx = format!(
        "failed to generate the output interface pairs for container id {}",
        &container.id
    );
    let interfaces = container.interfaces().context(ctx)?;
    Ok(Output {
        container,
        interfaces,
    })
}

/// Pretty print the error and exit with code `1`
fn pretty_print_err_and_exit(e: Error) {
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

/// Print the error in JSON format and exit with code `1`
fn print_err_as_json_and_exit(e: Error) {
    let mut fail: &Fail = e.cause();
    let mut caused_by = vec![];
    while let Some(cause) = fail.cause() {
        caused_by.push(cause.to_string());
        fail = cause;
    }

    let err_str = json!({
        "error": e.cause().to_string(),
        "caused_by": caused_by
    });

    println!("{}", err_str);
    std::process::exit(1);
}

/// Pretty print the output and exit with code `0`
fn pretty_print_output_and_exit(output: Vec<Output>) {
    let mut r = vec![];

    if output.len() > 0 {
        let l = "CONTAINER_ID\tNODE\tINTERFACE\tMTU\tMAC_ADDRESS\tBRIDGE".to_string();
        r.push(l);
    }

    for i in output {
        for intf in i.interfaces {
            let l = format!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                &i.container.id[0..12],
                i.container.node_name.as_ref().map_or("-", |s| &s[..]),
                &intf.node.name,
                &intf.node.mtu,
                &intf.node.mac_address,
                intf.node.bridge.as_ref().map_or("-", |s| &s[..])
            );
            r.push(l);
        }
    }
    let output_string = r.join("\n");
    let tw = TabWriter::new(Vec::<u8>::new());
    println!(
        "\n{}\n",
        tabify(tw, &output_string[..]).expect("failed to format the output")
    );
    std::process::exit(0);
}

/// Align the tab separated values to make them look nice
pub fn tabify(mut tw: TabWriter<Vec<u8>>, s: &str) -> Result<String, Error> {
    write!(&mut tw, "{}", s)?;
    tw.flush()?;
    Ok(String::from_utf8(tw.into_inner()?)?)
}

/// The output data structure
#[derive(Debug, Serialize)]
struct Output {
    container: Container,
    interfaces: Vec<VethIntfPair>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct VethIntf {
    name: String,
    ifindex: u16,
    peer_ifindex: u16,
    mtu: u16,
    mac_address: String,
    bridge: Option<String>,
    ip_address: Option<String>,
}

// a pair of container/node interfaces, e.g. a veth pair
#[derive(Debug, Serialize)]
struct VethIntfPair {
    container: VethIntf,
    node: VethIntf,
}

#[derive(Debug, Serialize)]
pub enum ContainerRuntime {
    Docker,
}

#[derive(Debug, Serialize)]
pub struct Container {
    pub id: String,
    pub pid: u32,
    pub node_name: Option<String>,
    pub runtime: ContainerRuntime,
}

impl Container {
    fn new(id: String, runtime: ContainerRuntime) -> Result<Self, Error> {
        // Retrieve the `pid` of the container
        let pid = match runtime {
            ContainerRuntime::Docker => {
                // fetch the PID using docker CLI
                // a docker client is not currently used as it's hard to find a lightweight
                // and good enough one in the rust ecosystem
                debug!("trying to find the pid for docker container {}", &id);
                let cmd = format!("docker inspect {} --format '{{{{.State.Pid}}}}'", &id);
                let output = run_host_cmd(&cmd)?;
                let pid: u32 = output.trim_matches('\'').parse()?;
                pid
            }
        };

        let container = Self {
            id,
            pid,
            runtime,
            node_name: None,
        };
        debug!("new Container: {:?}", &container);
        Ok(container)
    }

    /// Get the list of container interfaces
    fn get_container_interfaces(&self) -> Result<Vec<VethIntf>, Error> {
        debug!(
            "fetching `ip addr show` printout for container {}",
            &self.id
        );
        let cmd = format!("nsenter -t {} -n -- ip addr show", &self.pid);
        let output = run_host_cmd(&cmd)?;

        parse_ip_link_or_addr_printout(&output)
    }

    /// create a list of interface pairs,
    /// i.e. the container interfaces and their corresponding node interface
    fn interfaces(&self) -> Result<Vec<VethIntfPair>, Error> {
        // fetch the node interfaces
        debug!("fetching node `ip link show` printout");
        let cmd = "ip link show";
        let output = run_host_cmd(cmd)?;

        let mut node_intfs = parse_ip_link_or_addr_printout(&output)?;

        let container_intfs = self.get_container_interfaces()?;

        let mut out = vec![];

        // group the container interface and the corresponding node interface

        // Rust does not allow to take out elements of a vec while iterating through it
        // so find the index of the node interface for every container interface
        // and use the index to extract the needed element
        for cintf in container_intfs {
            let err = error::IntfMissingErr(cintf.peer_ifindex);
            let pos = node_intfs
                .iter()
                .position(|nintf| cintf.peer_ifindex == nintf.ifindex)
                .ok_or(err)?;
            let nintf = node_intfs.swap_remove(pos);
            out.push(VethIntfPair {
                container: cintf,
                node: nintf,
            });
        }
        Ok(out)
    }
}

/// Parse the output of `ip link show` or `ip addr show` and extract the interfaces
fn parse_ip_link_or_addr_printout(printout: &str) -> Result<Vec<VethIntf>, Error> {
    debug!("parsing ip link/addr printout");
    let mut res = vec![];

    lazy_static! {
        static ref S: &'static str = concat!(
            r"(?P<index>\d+):\s+(?P<name>\w+)@if(?P<pindex>\d+):",
            r".*\s+mtu\s+(?P<mtu>\d+)\s+",
            r"(?:.*\s+master\s+(?P<br>\S+)\s+)?",
            r".*\s+link/ether\s+(?P<mac>(\S)+)\s+",
            r"(.*\s+inet\s+(?P<ipv4>\S+)\s+)?",
        );
        static ref RE: Regex = Regex::new(&S).unwrap();
    }
    let err = error::IpLinkOrAddrShowParseErr;
    for m in RE.captures_iter(printout) {
        let intf = VethIntf {
            name: m.name("name").ok_or(err)?.as_str().to_string(),
            ifindex: m.name("index").ok_or(err)?.as_str().parse()?,
            peer_ifindex: m.name("pindex").ok_or(err)?.as_str().parse()?,
            mtu: m.name("mtu").ok_or(err)?.as_str().parse()?,
            bridge: m.name("br").map(|v| v.as_str().to_string()),
            mac_address: m.name("mac").ok_or(err)?.as_str().to_string(),
            ip_address: m.name("ipv4").map(|m| m.as_str().to_string()),
        };
        res.push(intf);
    }
    if res.len() == 0 {
        Err(err)?
    } else {
        Ok(res)
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
