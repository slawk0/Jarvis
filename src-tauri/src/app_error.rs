#[derive(serde::Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AppError {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl AppError {
    pub const TRANSFER_CANCELLED: &'static str = "TRANSFER_CANCELLED";

    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            details: None,
        }
    }

    pub fn with_details(code: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            details: Some(details.into()),
        }
    }

    pub fn is_transfer_cancelled(&self) -> bool {
        self.code == Self::TRANSFER_CANCELLED
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| self.code.clone())
    }
}

impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_json()
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)
    }
}
