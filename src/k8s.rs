use super::error::K8sError;
use failure::Error;
use kubeclient::prelude::*;
use kubeclient::resources::Pod;
/// Module used to handle Kubernetes specifics
use std::env;

/// Return the path to the config file as String
///
/// By default, look for a file named `config` in the `$HOME/.kube` directory.
/// The user can specify other kubeconfig files by setting the `KUBECONFIG` environment variable
fn get_kubeconfig_path() -> Result<String, K8sError> {
    let key = "KUBECONFIG";
    match env::var(key) {
        Ok(val) => {
            debug!("using kubeconfig from ${}: {}", key, val);
            Ok(val)
        }
        Err(_) => {
            // use `$HOME/.kube/config` if it exist
            if let Some(dir) = env::home_dir() {
                let cfg_file_path = dir.join(".kube/config");
                match cfg_file_path.is_file() {
                    true => {
                        let cfg = cfg_file_path.to_string_lossy().to_string();
                        debug!("using kubeconfig: {}", cfg);
                        Ok(cfg)
                    }
                    false => Err(K8sError::KubeconfigMissing),
                }
            } else {
                warn!("could not find the user home directory");
                Err(K8sError::KubeconfigMissing)
            }
        }
    }
}


/// Fetch the k8s pod with the given name on the given namespace (the `ns` argument).
/// Namespace `default` is used if the namespace is not specified (`ns` argument is `None`)
pub(crate) fn get_pod(name: String, ns: Option<String>) -> Result<Pod, Error> {
    let cfg = get_kubeconfig_path()?;
    let kube =
        Kubernetes::load_conf(&cfg).map_err(|e| K8sError::KubeclientError { e: e.to_string() })?;

    // use `default` if no other namespace is specified
    let ns = match ns {
        Some(ref ns) => &ns[..],
        None => "default",
    };

    debug!("getting pod {} on namespace {}", &name, ns);

    let pod = kube.namespace(ns)
        .pods()
        .get(&name[..])
        .map_err(|e| K8sError::KubeclientError { e: e.to_string() })?;

    trace!("k8s response for pod {} on namespace {}:\n{:#?}", &name, ns, pod);

    Ok(pod)
}

enum ContainerRuntime {
    Docker,
    Rkt
}

struct Container {
    id: String,
    name: String,
    node: String,
    runtime: ContainerRuntime
}

/// Extract the IDs of the containers part of the given pod
fn extract_container_info(pod: Pod) -> Result<Vec<Container>, K8sError> {
    match pod.status {
        Some(pod_status) => {
            match pod_status.container_statuses {
                Some(cs) => {Ok(vec![])},
                None => {
                    debug!("pod.status.container_statuses is None");
                    Err(K8sError::ContainerDataNotFound)
                }
            }
        },
        None => {
            debug!("pod.status is None");
            Err(K8sError::ContainerDataNotFound)
        }
    }
}