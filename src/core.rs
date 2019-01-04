extern crate pnet;
extern crate pnetlink;

use std::net::IpAddr;
use pnet::util::MacAddr;
use pnetlink::packet::netlink::NetlinkConnection;
use pnetlink::packet::route::link::{Links,Link, OperState, IfType, LinkType};
use pnetlink::packet::route::addr::{Addresses,Addr};
use netns::NetNS;


/// Store the parts of a linux Link (interface) that we care about
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Intf {
    ifindex: u32,
    iftype: IfType,
    linktype: LinkType,
    state: OperState,
    name: Option<String>,
    peer_ifindex: Option<u16>,
    mtu: Option<u32>,
    mac_address: Option<MacAddr>,
    ip_addresses: Vec<IPAddress>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum IntfKind {
    Veth,
    Macvlan(MacVlanInfo),
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct MacVlanInfo {
    mode: MacVlanMode,
    master: String

}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum MacVlanMode {
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
    pub pid: i32,
    node_name: Option<String>,
    pub runtime: ContainerRuntime,
}

#[derive(Debug, Serialize)]
pub struct IPAddress {
    pub ip: IpAddr,
    pub prefix_len: u8
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


/// Retrieve all interfaces in the given netns if specified or the default one otherwise
pub fn get_intfs(netns_pid: Option<i32>) -> Result<Vec<Intf>, Error> {
    intfs = vec![];

    if let Some(pid) = netns_pid {
        let ns = NetNS::get_from_process(pid).unwrap();
        debug!("fetching all interfaces in netns {:#?}", &ns);
        NetNS::set(ns)?;
    }

    let mut conn = NetlinkConnection::new();
    let links = conn.iter_links().unwrap().collect::<Vec<_>>();
    for link in links {
        let mut ip_addrs = vec![];
        for addr in conn.get_link_addrs(None, &link).unwrap() {
            let addr = IPAddress{
                ip: addr.get_ip(),
                prefix_len: addr.get_prefix_len(),
            }:
            ip_addrs.push(addr);
        }

        let intf = Intf {
            ifindex: link.get_index(),
            iftype: link.get_type(),
            state: link.get_state(),
            name: link.get_name(),
            peer_ifindex: Option<u16>,
            mtu: link.get_mtu(),
            kind: IntfKind,
            mac_address: link.get_hw_addr(),
            ip_addresses: ip_addrs,
        }
    }

}

/// Retrieve the interface with the given id in the given netns
/// if specified or the default one otherwise
pub fn get_intf(netns_pid: Option<i32>) -> Result<Vec<Intf>, Error> {
    unimplemented!()
}

