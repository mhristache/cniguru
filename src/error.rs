/// Wrapper error type to get `kubeclient-rs` to work with `failure-rs`
#[derive(Debug, Fail)]
pub enum K8sError {
    #[fail(display = "{}", e)]
    KubeclientError { e: String },

    #[fail(display = "kubernetes config not found")]
    KubeconfigMissing,

    #[fail(display = "could not find needed info about containers in the pod data")]
    ContainerDataNotFound
}
