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
                serde_json::to_string_pretty(&v).expect("failed to serialize the output to json")
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
        let l =
            "CONTAINER_ID\tPID\tNODE\tINTF(C)\tMAC_ADDRESS(C)\tIP_ADDRESS(C)\tINTF(N)\tBRIDGE(N)"
                .to_string();
        r.push(l);
    }

    for i in output {
        let short_id = &i.container.id[0..12];
        for intf in i.interfaces {
            let l = format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                short_id,
                i.container.pid,
                i.container.node_name.as_ref().map_or("-", |s| &s[..]),
                &intf.container.name,
                &intf.container.mac_address,
                &intf.container.ip_address.as_ref().map_or("-", |s| &s[..]),
                &intf.node.name,
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

