use anyhow::{bail, Context, Result};
use container_pid::lookup_container_pid;
use docopt::Docopt;
use futures::stream::TryStreamExt;
use k8s_openapi::api::core::v1::Pod;
use log::debug;
use netns::NetNS;
use rtnetlink::{
    packet::rtnl::{
        constants::{AF_BRIDGE, RTEXT_FILTER_BRVLAN},
        link::nlas::{Info, InfoBridge, InfoData, InfoIpVlan, InfoKind, InfoMacVlan, Nla, State},
    },
    Handle,
};
use serde::Deserialize;
use serde::Serialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use sys_mount::{Mount, MountFlags, Unmount, UnmountFlags};
use tokio::fs;
use url::Url;

const USAGE: &'static str = "
Usage: cniguru pod <id> [-n <namespace> ] [-o <output>]
       cniguru container <id> [-o <output> ]
       cniguru [-h] [--version]

Options:
    -h, --help         Show this message.
    --version          Show the version
    -n <namespace>     Specify a kubernetes namespace

Main commands:
    pod                The name of a kubernetes pod
    container          The name or id of a container (e.g. docker/podman/rkt etc container)
";

#[derive(Debug, Deserialize)]
struct Args {
    cmd_pod: bool,
    cmd_container: bool,
    arg_id: String,
    flag_n: Option<String>,
    flag_version: bool,
}

#[derive(Serialize)]
struct Output {
    host_network: bool,
    container: String,
    pid: i32,
    interfaces: Vec<Interface>,
}

#[derive(Serialize)]
struct Interface {
    name: Option<String>,
    index: u32,
    oper_state: Option<String>,
    mtu: Option<u32>,
    mac_address: Option<Vec<u8>>,
    kind: Option<String>,
}

#[derive(Debug, Serialize)]
struct EthDev {
    name: OsString,
    index: i32,
    driver: OsString,
    pci_id: OsString,
    numa_node: i8,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    debug!("program args: {:?}", args);

    if args.flag_version {
        println!("{}", "0.3.0");
        return Ok(());
    }

    let container;

    if args.cmd_pod {
        let client = kube::Client::try_default().await?;
        let ns = args
            .flag_n
            .as_ref()
            .map(String::as_str)
            .unwrap_or("default");
        let pods: kube::api::Api<Pod> = kube::api::Api::namespaced(client, ns);
        let pod = pods.get(&*args.arg_id).await?;
        container = get_id_of_first_container_in_pod(&pod)?;
    } else if args.cmd_container {
        container = args.arg_id;
    } else {
        println!("Not enough arguments.\n{}", &USAGE);
        std::process::exit(1);
    }
    let pid = lookup_container_pid(&*container, &vec![])?;

    let container_netns = get_netns_for_pid(pid).await?;
    let default_netns = get_netns_for_pid(1).await?;

    let netns = NetNS::get_from_process(pid)?;
    debug!("entering netns for pid {}", pid);
    NetNS::set(netns)?;

    let (connection, handle, _) = rtnetlink::new_connection()?;
    tokio::spawn(connection);

    let interfaces = get_links(handle.clone()).await?;

    let output = Output {
        container,
        interfaces,
        pid,
        host_network: container_netns == default_netns,
    };

    let eth_devs = get_eth_devices().await?;
    println!("{:#?}", eth_devs);

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

// Extract the ID of the container part of the given pod
fn get_id_of_first_container_in_pod(pod: &Pod) -> Result<String> {
    // extract the first container in the pod
    if let Some(container_status) = pod
        .status
        .as_ref()
        .and_then(|pod_status| pod_status.container_statuses.as_ref())
        .and_then(|containers_status| containers_status.first())
    {
        let container_id = container_status
            .container_id
            .as_ref()
            .expect("container id is missing");
        // the containerID is expected to have an URL format, e.g. docker://c6671e7930e7181d7e..
        let container_id = Url::parse(container_id)?;
        if let Some(id) = container_id.host_str() {
            Ok(id.to_string())
        } else {
            bail!("count not extract the container id from {}", container_id);
        }
    } else {
        bail!("no container found in pod");
    }
}

/// Find the default network namespace
pub async fn get_netns_for_pid(pid: i32) -> Result<PathBuf> {
    let path = format!("/proc/{}/ns/net", pid);
    Ok(fs::read_link(path).await?)
}

async fn get_links(handle: Handle) -> Result<Vec<Interface>> {
    let mut res = vec![];
    let mut links = handle.link().get().execute();
    while let Some(msg) = links.try_next().await? {
        // only care about ethernet links
        if msg.header.link_layer_type != 1 {
            debug!(
                "ignoring intf {} with link_layer_type {}",
                msg.header.index, msg.header.link_layer_type
            );
            continue;
        }
        let (mut name, mut oper_state, mut mac_address, mut kind, mut mtu) =
            (None, None, None, Some("Phys".to_string()), None);
        for nla in msg.nlas.into_iter() {
            match nla {
                Nla::IfName(n) => {
                    name.replace(n);
                }
                Nla::Address(a) => {
                    mac_address.replace(a);
                }
                Nla::Mtu(m) => {
                    mtu.replace(m);
                }
                Nla::OperState(s) => {
                    oper_state.replace(format!("{:?}", s));
                }
                Nla::Info(info) => {
                    for i in info {
                        match i {
                            Info::Kind(k) => {
                                kind.replace(format!("{:?}", k));
                            }
                            _ => continue,
                        }
                    }
                }
                _ => continue,
            }
        }
        let intf = Interface {
            name,
            oper_state,
            mac_address,
            kind,
            mtu,
            index: msg.header.index,
        };
        res.push(intf);
    }
    Ok(res)
}

// return a list of ethernet devices (interfaces which are connected to a pci device)
// use the same logic as in eth_info.sh script to find the eth devices attached to the container
// https://github.com/mhristache/lxtools/blob/master/eth_info.sh
async fn get_eth_devices() -> Result<Vec<EthDev>> {
    // to get the list of devices allocated to a netns
    // we need to mount sysfs while attached to the netns
    let mnt_path = Path::new("/tmp/cniguru");
    fs::create_dir_all(mnt_path).await?;

    let _mount = Mount::new("/sys", mnt_path, "sysfs", MountFlags::empty(), None)?
        .into_unmount_drop(UnmountFlags::empty());

    let mut res = vec![];
    let pci_dev_path = mnt_path.join("bus/pci/devices");

    let mut entries = fs::read_dir(pci_dev_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let class = fs::read(entry.path().join("class")).await?;

        // we only care about ethernet devices
        if &class[..] == b"0x020000\n" {
            let mut intf_name = None;
            let mut intf_path = None;

            'upper: for p in ["net", "uio"] {
                if let Ok(mut net_entries) = fs::read_dir(entry.path().join(p)).await {
                    while let Some(net_entry) = net_entries.next_entry().await? {
                        intf_name.replace(net_entry.file_name());
                        intf_path.replace(net_entry.path());
                        // we don't expect more than one entry
                        break 'upper;
                    }
                }
            }

            if let (Some(name), Some(path)) = (intf_name, intf_path) {
                let p = entry.path().join("numa_node");
                let content = fs::read(&p)
                    .await
                    .with_context(|| format!("error reading {}", p.display()))?;
                let numa_node: i8 =
                    String::from_utf8_lossy(&content[..content.len() - 1]).parse()?;

                let p = entry.path().join("driver");
                let driver = fs::read_link(&p)
                    .await
                    .with_context(|| format!("error reading {} symlink", p.display()))?;
                let driver = driver.file_name().expect("invalid driver file symlink");

                let p = path.join("ifindex");
                let content = fs::read(&p)
                    .await
                    .with_context(|| format!("error reading {}", p.display()))?;
                let index: i32 = String::from_utf8_lossy(&content[..content.len() - 1]).parse()?;

                let intf = EthDev {
                    name,
                    index,
                    pci_id: entry.file_name(),
                    driver: driver.to_os_string(),
                    numa_node,
                };

                res.push(intf);
            }
        }
    }
    Ok(res)
}
