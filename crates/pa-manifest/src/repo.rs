/// Um repositório do GitHub, de qualquer jeito que a pessoa colar o link.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
}

impl RepoRef {
    /// Aceita `https://github.com/dono/repo`, com `.git`, barra final, `/tree/branch`, `?query`, sem
    /// `https://`, ou só `dono/repo`.
    pub fn parse(input: &str) -> Option<Self> {
        let s = input.trim();
        let s = s.strip_prefix("git+").unwrap_or(s);
        let s = ["https://", "http://", "ssh://git@", "git@"].iter().find_map(|p| s.strip_prefix(p)).unwrap_or(s);
        let s = s.split(['?', '#']).next().unwrap_or("");
        let path = match s.strip_prefix("www.github.com").or_else(|| s.strip_prefix("github.com")) {
            Some(rest) => rest.trim_start_matches([':', '/']),
            None if s.contains('.') && s.split('/').next().is_some_and(|h| h.contains('.')) => return None,
            None => s,
        };
        let mut parts = path.split('/').filter(|p| !p.is_empty());
        let owner = parts.next()?;
        let name = parts.next()?.trim_end_matches(".git");
        let valid = |p: &str| {
            !p.is_empty() && p.len() <= 100 && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        };
        (valid(owner) && valid(name) && name != "." && name != "..")
            .then(|| Self { owner: owner.to_string(), name: name.to_string() })
    }

    pub fn url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.name)
    }

    pub fn api(&self, path: &str) -> String {
        format!("https://api.github.com/repos/{}/{}/{}", self.owner, self.name, path.trim_start_matches('/'))
    }

    /// Arquivo cru da branch padrão (`HEAD`).
    pub fn raw(&self, path: &str) -> String {
        format!("https://raw.githubusercontent.com/{}/{}/HEAD/{}", self.owner, self.name, path.trim_start_matches('/'))
    }
}

impl std::fmt::Display for RepoRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_what_people_paste() {
        let want = RepoRef { owner: "joao".into(), name: "arena-blast".into() };
        for input in [
            "https://github.com/joao/arena-blast",
            "https://github.com/joao/arena-blast/",
            "https://github.com/joao/arena-blast.git",
            "https://github.com/joao/arena-blast/tree/main/assets",
            "https://www.github.com/joao/arena-blast?tab=readme",
            "github.com/joao/arena-blast",
            "git@github.com:joao/arena-blast.git",
            "  joao/arena-blast  ",
        ] {
            assert_eq!(RepoRef::parse(input).as_ref(), Some(&want), "{input}");
        }
    }

    #[test]
    fn rejects_other_hosts_and_junk() {
        for input in ["https://gitlab.com/joao/x", "joao", "", "https://github.com/joao", "a/b c", "x.com/a/b"] {
            assert_eq!(RepoRef::parse(input), None, "{input}");
        }
    }

    #[test]
    fn builds_urls() {
        let r = RepoRef::parse("joao/arena").unwrap();
        assert_eq!(r.api("releases/latest"), "https://api.github.com/repos/joao/arena/releases/latest");
        assert_eq!(r.raw("party-anywhere.json"), "https://raw.githubusercontent.com/joao/arena/HEAD/party-anywhere.json");
    }
}
