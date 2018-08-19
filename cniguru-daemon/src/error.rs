use kubeclient;

#[derive(Debug, Fail)]
pub enum K8sError {
    #[fail(display = "{}", _0)]
    KubeclientError(String),

    #[fail(
        display = "kubernetes config not specified using $KUBECONFIG env var and could not open either $HOME/.kube/config or /etc/kubernetes/admin.conf"
    )]
    KubeconfigMissing,

    #[fail(display = "container has an unsupported runtime: {}", _0)]
    UnsupportedContainerRuntime(String),

    #[fail(display = "field {} has an unsupported format: {}", field, val)]
    UnsupportedFieldFormat { field: String, val: String },

    #[fail(display = "field {} is missing or is null", _0)]
    MissingOrNullField(String),
}

impl From<kubeclient::errors::Error> for K8sError {
    fn from(err: kubeclient::errors::Error) -> K8sError {
        K8sError::KubeclientError(err.to_string())
    }
}

#[derive(Debug, Fail)]
pub enum HostCmdError {
    #[fail(display = "command'{}' failed with code {}: {}", cmd, code, stderr)]
    CmdFailed {
        cmd: String,
        code: String,
        stderr: String,
    },

    #[fail(display = "invalid command: '{}'", _0)]
    CmdInvalid(String),
}

#[derive(Debug, Fail, Copy, Clone)]
#[fail(display = "failed to extract veth interfaces from the output of `ip link/addr show`")]
pub struct IpLinkOrAddrShowParseErr;

#[derive(Debug, Fail, Copy, Clone)]
#[fail(display = "failed to find a node veth interface with ifindex {}", _0)]
pub struct IntfMissingErr(pub u16);
