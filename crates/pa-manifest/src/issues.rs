use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// O Hub recusa o jogo.
    Error,
    /// Funciona, mas vale corrigir.
    Warning,
}

/// Um problema encontrado, com o campo e uma mensagem que diz o que fazer (UX: nenhum erro sem saída).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub severity: Severity,
    /// Caminho do campo no JSON, ex.: `players.max`.
    pub field: String,
    pub message: String,
}

impl Issue {
    pub fn error(field: &str, message: impl Into<String>) -> Self {
        Self { severity: Severity::Error, field: field.into(), message: message.into() }
    }

    pub fn warning(field: &str, message: impl Into<String>) -> Self {
        Self { severity: Severity::Warning, field: field.into(), message: message.into() }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tag = match self.severity {
            Severity::Error => "erro",
            Severity::Warning => "aviso",
        };
        write!(f, "{tag} em {}: {}", self.field, self.message)
    }
}
