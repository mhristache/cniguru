use super::error::K8sError;
use super::LinuxNamespace;
use failure::{Error, ResultExt};
use kubeclient::{self, prelude::*};
use std::env;
use url::Url;

pub struct Pod<'a> {
    name: &'a str,
    namespace: &'a str,
}

impl<'a> Pod<'a> {
    pub fn new(name: &'a str, namespace: Option<&'a str>) -> Self {
        // use `default` if no other namespace is specified
        let namespace = match namespace {
            Some(ns) => ns,
            None => "default",
        };
        debug!(" pod {}, namespace {}", name, namespace);
        Self { name, namespace }
    }

    /// Return the path to the config file as String
    ///
    /// By default, look for a file named `config` in the `$HOME/.kube` directory.
    /// The user can specify other kubeconfig files by setting the `KUBECONFIG` environment variable
    fn get_kubeconfig_path(&self) -> Result<String, K8sError> {
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

    /// Fetch the k8s pod with the given name on the given namespace
    fn get_pod(&self) -> Result<kubeclient::resources::Pod, K8sError> {
        let cfg = self.get_kubeconfig_path()?;
        let kube = Kubernetes::load_conf(&cfg)?;

        let pod = kube.namespace(self.namespace).pods().get(self.name)?;

        trace!("k8s response:\n{:#?}", pod);

        Ok(pod)
    }

    pub fn containers(&self) -> Result<Vec<Container>, Error> {
        let pod = self.get_pod()?;
        extract_container_info(pod)
    }
}

impl LinuxNamespace for Container {
    fn pid() -> u32 {
        unimplemented!()
    }
}

#[derive(Debug)]
pub enum ContainerRuntime {
    Docker,
}

#[derive(Debug)]
pub struct Container {
    id: String,
    node_name: String,
    runtime: ContainerRuntime,
}

/// Extract the IDs of the containers part of the given pod
fn extract_container_info(pod: kubeclient::resources::Pod) -> Result<Vec<Container>, Error> {
    let mut res = vec![];
    match pod.status {
        Some(pod_status) => {
            match pod_status.container_statuses {
                Some(objs) => {
                    for (idx, obj) in objs.iter().enumerate() {
                        // the json path to the object, used for details about errors
                        let obj_path = format!("pod.status.container_statuses.{}", idx);
                        let (runtime, container_id) =
                            match obj.get("containerID").and_then(|x| x.as_str()) {
                                Some(raw_cid) => {
                                    // the containerID is expected to have an URL format
                                    // e.g. docker://c6671e7930e7181d7e..
                                    let cid = Url::parse(raw_cid)?;

                                    let runtime = match cid.scheme() {
                                        "docker" => ContainerRuntime::Docker,
                                        other @ _ => {
                                            let ctx = format!(
                                                "{}.containerID has an unsupported runtime: {}",
                                                &obj_path, other
                                            );
                                            Err(K8sError::PodContainerDataError).context(ctx)?
                                        }
                                    };

                                    let id = match cid.host_str() {
                                        Some(s) => s.to_string(),
                                        None => {
                                            let ctx = format!(
                                                "{}.containerID has an unsupported format: {}",
                                                &obj_path, &raw_cid
                                            );
                                            Err(K8sError::PodContainerDataError).context(ctx)?
                                        }
                                    };

                                    (runtime, id)
                                }
                                None => {
                                    let ctx = format!("{}.containerID is null", &obj_path);
                                    Err(K8sError::PodContainerDataError).context(ctx)?
                                }
                            };
                        let entry = Container {
                            id: container_id,
                            node_name: pod
                                .spec
                                .node_name
                                .as_ref()
                                .expect("pod.spec.node_name is null")
                                .clone(),
                            runtime: runtime,
                        };
                        res.push(entry);
                    }
                }
                None => {
                    Err(K8sError::PodContainerDataError)
                        .context("pod.status.container_statuses is null")?;
                }
            }
        }
        None => {
            Err(K8sError::PodContainerDataError).context("pod.status is null")?;
        }
    }
    Ok(res)
}
