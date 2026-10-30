//! HTTP atrás de uma interface: os testes usam respostas prontas, o Hub e a CLI usam o `ureq`.

use std::io::Read;

/// Limite para arquivos JSON (manifesto, anexo, respostas da API).
pub const MAX_JSON_BYTES: u64 = 2 * 1024 * 1024;
/// Limite para pacotes de jogo.
pub const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
    pub etag: Option<String>,
}

impl Response {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

pub trait Http {
    /// GET com `If-None-Match` opcional (304 = nada mudou e não gasta a cota da API do GitHub).
    fn get(&self, url: &str, etag: Option<&str>, max_bytes: u64) -> Result<Response, String>;
}

/// Cliente real. Token opcional (`GITHUB_TOKEN`) só aumenta a cota da API; nada exige login.
pub struct Ureq {
    agent: ureq::Agent,
    token: Option<String>,
}

impl Default for Ureq {
    fn default() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(10))
            .timeout_read(std::time::Duration::from_secs(60))
            .user_agent(concat!("party-anywhere/", env!("CARGO_PKG_VERSION")))
            .build();
        let token = std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.trim().is_empty());
        Self { agent, token }
    }
}

impl Http for Ureq {
    fn get(&self, url: &str, etag: Option<&str>, max_bytes: u64) -> Result<Response, String> {
        let mut req = self.agent.get(url);
        if url.starts_with("https://api.github.com/") {
            req = req.set("Accept", "application/vnd.github+json").set("X-GitHub-Api-Version", "2022-11-28");
            if let Some(t) = &self.token {
                req = req.set("Authorization", &format!("Bearer {t}"));
            }
        }
        if let Some(e) = etag {
            req = req.set("If-None-Match", e);
        }
        let resp = match req.call() {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(ureq::Error::Transport(t)) => return Err(format!("sem conexão com {}: {t}", host(url))),
        };
        let status = resp.status();
        let etag = resp.header("ETag").map(str::to_string);
        let mut body = Vec::new();
        resp.into_reader()
            .take(max_bytes + 1)
            .read_to_end(&mut body)
            .map_err(|e| format!("download interrompido: {e}"))?;
        if body.len() as u64 > max_bytes {
            return Err("arquivo maior do que o permitido".into());
        }
        Ok(Response { status, body, etag })
    }
}

fn host(url: &str) -> &str {
    url.split('/').nth(2).unwrap_or(url)
}

#[cfg(test)]
pub mod fake {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    /// Respostas prontas por URL; registra o que foi pedido.
    #[derive(Default)]
    pub struct FakeHttp {
        pub routes: HashMap<String, Response>,
        pub calls: RefCell<Vec<(String, Option<String>)>>,
    }

    impl FakeHttp {
        pub fn on(&mut self, url: &str, status: u16, body: impl Into<Vec<u8>>) -> &mut Self {
            self.routes.insert(url.to_string(), Response { status, body: body.into(), etag: Some(format!("\"{url}\"")) });
            self
        }
    }

    impl Http for FakeHttp {
        fn get(&self, url: &str, etag: Option<&str>, _max: u64) -> Result<Response, String> {
            self.calls.borrow_mut().push((url.to_string(), etag.map(str::to_string)));
            match self.routes.get(url) {
                Some(r) if etag.is_some() && etag == r.etag.as_deref() => Ok(Response { status: 304, body: vec![], etag: r.etag.clone() }),
                Some(r) => Ok(r.clone()),
                None => Ok(Response { status: 404, body: b"{\"message\":\"Not Found\"}".to_vec(), etag: None }),
            }
        }
    }
}
