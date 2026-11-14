use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use thiserror::Error;

/// 所有命令的统一错误类型。前端收到的形状固定为 `{ code, message }`，
/// 对应 `apps/desktop/src/lib/ipc.ts` 里的 `CommandError`。
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    InvalidInput(String),

    /// 自更新过程中的失败（查不到、下载失败、装不上）。文案已经是给人看的。
    #[error("{0}")]
    Update(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

impl AppError {
    /// 稳定的机器可读错误码；前端按它分支，`message` 只用于展示。
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "invalid_input",
            Self::Update(_) => "update",
            Self::Io(_) | Self::Tauri(_) => "internal",
        }
    }
}

// 前端是同一个应用的一部分，可信，因此内部错误也把原文带过去，方便排查。
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.code() == "internal" {
            log::error!("{self}");
        }

        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::AppError;

    #[test]
    fn serializes_to_code_and_message() {
        let json = serde_json::to_value(AppError::InvalidInput("名字不能为空".to_owned()))
            .expect("serializable");
        assert_eq!(
            json,
            serde_json::json!({ "code": "invalid_input", "message": "名字不能为空" })
        );
    }

    #[test]
    fn io_errors_are_internal() {
        let error = AppError::from(std::io::Error::other("disk on fire"));
        assert_eq!(error.code(), "internal");
    }
}
