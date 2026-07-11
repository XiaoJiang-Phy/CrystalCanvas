#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcErrorCode {
    InvalidArgument,
    IoError,
    LockPoisoned,
    ParseError,
    RenderError,
    InternalError,
}

#[derive(Debug, serde::Serialize)]
pub struct IpcError {
    pub code: IpcErrorCode,
    pub message: String,
    pub recoverable: bool,
}

pub type IpcResult<T> = Result<T, IpcError>;

impl IpcError {
    pub fn new(code: IpcErrorCode, message: impl Into<String>, recoverable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            recoverable,
        }
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(IpcErrorCode::InvalidArgument, message, true)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(IpcErrorCode::IoError, message, true)
    }

    pub fn lock(message: impl Into<String>) -> Self {
        Self::new(IpcErrorCode::LockPoisoned, message, false)
    }

    pub fn parse(message: impl Into<String>) -> Self {
        Self::new(IpcErrorCode::ParseError, message, true)
    }

    pub fn render(message: impl Into<String>) -> Self {
        Self::new(IpcErrorCode::RenderError, message, true)
    }
}

impl From<String> for IpcError {
    fn from(message: String) -> Self {
        Self::new(IpcErrorCode::InternalError, message, false)
    }
}

impl From<&str> for IpcError {
    fn from(message: &str) -> Self {
        Self::new(IpcErrorCode::InternalError, message, false)
    }
}

#[cfg(test)]
mod tests {
    use super::{IpcError, IpcErrorCode};

    #[test]
    fn ipc_error_has_stable_wire_shape() {
        let value = serde_json::to_value(IpcError::new(
            IpcErrorCode::InvalidArgument,
            "unsupported format",
            true,
        ))
        .expect("IpcError must serialize");

        assert_eq!(
            value,
            serde_json::json!({
                "code": "invalid_argument",
                "message": "unsupported format",
                "recoverable": true
            })
        );
    }
}
