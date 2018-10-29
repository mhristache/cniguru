extern crate pnet;
extern crate pnetlink;

use std::net::IpAddr;
use pnet::util::MacAddr;

/// Store the parts of a linux Link (interface) that we care about
#[derive(Debug, PartialEq, Eq, Serialize)]
struct Intf {
    name: String,
    ifindex: u16,
    peer_ifindex: Option<u16>,
    mtu: u16,
    kind: IntfKind,
    mac_address: Option<MacAddr>,
    ip_address: Option<IpAddr>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
enum IntfKind {
    Veth,
    Macvlan(MacVlanInfo),
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct MacVlanInfo {
    mode: MacVlanMode,

}

#[derive(Debug, PartialEq, Eq, Serialize)]
enum MacVlanMode {
    VEPA,
    Bridge,
    Passthru,
    Private,
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
    fn interfaces(&self) -> Result<Vec<Intf>, Error> {
        unimplemented!()
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
