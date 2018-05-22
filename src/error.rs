use kubeclient;

#[derive(Debug, Fail)]
pub enum K8sError {
    #[fail(display = "{}", _0)]
    KubeclientError(String),

    #[fail(display = "kubernetes config not found")]
    KubeconfigMissing,

    #[fail(display = "could not extract needed data about containers in the pod")]
    PodContainerDataError,
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

#[derive(Debug, Fail)]
pub enum DataExtractionError {
    #[fail(display = "failed to parse the output of '{}'", _0)]
    OutputParsingError(String),
}
