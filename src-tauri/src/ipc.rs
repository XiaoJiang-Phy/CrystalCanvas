#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcErrorCode {
    InvalidArgument,
    IoError,
    LockPoisoned,
    NotInTauri,
    StateBusy,
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

enum IpcEnumValue {
    Text(String),
    Invalid(&'static str),
}

pub struct IpcEnumInput<T> {
    value: IpcEnumValue,
    marker: std::marker::PhantomData<T>,
}

impl<'de, T> serde::Deserialize<'de> for IpcEnumInput<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(IpcEnumVisitor(std::marker::PhantomData))
    }
}

struct IpcEnumVisitor<T>(std::marker::PhantomData<T>);

impl<'de, T> serde::de::Visitor<'de> for IpcEnumVisitor<T> {
    type Value = IpcEnumInput<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a bounded IPC enum value")
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(IpcEnumInput {
            value: if value.len() <= 64 {
                IpcEnumValue::Text(value)
            } else {
                IpcEnumValue::Invalid("string longer than 64 bytes")
            },
            marker: std::marker::PhantomData,
        })
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value.len() > 64 {
            return Ok(invalid_ipc_enum_input("string longer than 64 bytes"));
        }
        Ok(IpcEnumInput {
            value: IpcEnumValue::Text(value.to_owned()),
            marker: std::marker::PhantomData,
        })
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
        Ok(invalid_ipc_enum_input("boolean"))
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
        Ok(invalid_ipc_enum_input("number"))
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
        Ok(invalid_ipc_enum_input("number"))
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
        Ok(invalid_ipc_enum_input("number"))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(invalid_ipc_enum_input("null"))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(invalid_ipc_enum_input("null"))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        while sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {}
        Ok(invalid_ipc_enum_input("array"))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        while map
            .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
            .is_some()
        {}
        Ok(invalid_ipc_enum_input("object"))
    }
}

fn invalid_ipc_enum_input<T>(kind: &'static str) -> IpcEnumInput<T> {
    IpcEnumInput {
        value: IpcEnumValue::Invalid(kind),
        marker: std::marker::PhantomData,
    }
}

impl<T> IpcEnumInput<T>
where
    T: serde::de::DeserializeOwned,
{
    pub fn parse(self, parameter: &str) -> IpcResult<T> {
        let value = match self.value {
            IpcEnumValue::Text(value) => value,
            IpcEnumValue::Invalid(kind) => {
                return Err(IpcError::invalid_argument(format!(
                    "invalid {}: expected string, received {}",
                    parameter, kind
                )));
            }
        };
        <T as serde::Deserialize>::deserialize(
            serde::de::value::StrDeserializer::<serde::de::value::Error>::new(&value),
        )
            .map_err(|error| IpcError::invalid_argument(format!("invalid {}: {}", parameter, error)))
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraAxis {
    A,
    B,
    C,
    AStar,
    BStar,
    CStar,
    Reset,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsosurfaceSignMode {
    Positive,
    Negative,
    Both,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeRenderMode {
    Isosurface,
    Volume,
    Both,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeColormap {
    Viridis,
    Grayscale,
    Inferno,
    Plasma,
    Coolwarm,
    Hot,
    Magma,
    Cividis,
    Turbo,
    Rdylbu,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    Openai,
    Deepseek,
    Claude,
    Gemini,
    Ollama,
}

impl LlmProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Deepseek => "deepseek",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::Ollama => "ollama",
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExportFileFormat {
    Poscar,
    Vasp,
    Lammps,
    Qe,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportImageBackground {
    Transparent,
    White,
    Black,
    Default,
}

impl ExportImageBackground {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Transparent => "transparent",
            Self::White => "white",
            Self::Black => "black",
            Self::Default => "default",
        }
    }
}

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

    pub fn busy(message: impl Into<String>) -> Self {
        Self::new(IpcErrorCode::StateBusy, message, true)
    }

    pub fn from_try_lock<T>(error: std::sync::TryLockError<T>, resource: &str) -> Self {
        match error {
            std::sync::TryLockError::WouldBlock => Self::busy(format!("{} is busy", resource)),
            std::sync::TryLockError::Poisoned(_) => Self::lock(format!("{} lock poisoned", resource)),
        }
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
