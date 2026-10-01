use std::io;
use std::path::{Path, PathBuf};

const MINILM_DIR: &str = "minilm-multilingual-q";
const ONNX_NAME: &str = "model_optimized.onnx";
const TOKENIZER_NAME: &str = "tokenizer.json";
const HF_BASE: &str =
    "https://huggingface.co/Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q/resolve/main";

/// Catalog entry for an on-demand embedding model (extensible in Phase 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbedModelSpec {
    pub id: &'static str,
    pub dir_name: &'static str,
    pub label: &'static str,
    pub aliases: &'static [&'static str],
    pub hf_base: &'static str,
    pub files: &'static [&'static str],
}

pub const MINILM_MULTILINGUAL_Q: EmbedModelSpec = EmbedModelSpec {
    id: "minilm-multilingual-q",
    dir_name: MINILM_DIR,
    label: "Paraphrase MiniLM multilingual Q (384-dim, recommended)",
    aliases: &["minilm", "mini-lm", "mini_lm", "minilm-q"],
    hf_base: HF_BASE,
    files: &[
        ONNX_NAME,
        TOKENIZER_NAME,
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ],
};

/// jinaai/jina-embeddings-v2-base-code, int8 ONNX (768-d): trained on code
/// and its documentation. Fused with the lexical ranking for plain-language
/// questions it lifts the ripgrep holdout from 0.500 to 0.667 recall
/// (`docs/measured.md`); MiniLM, a general paraphrase model, lowered it.
pub const JINA_CODE_V2: EmbedModelSpec = EmbedModelSpec {
    id: "jina-code-v2",
    dir_name: "jina-code-v2",
    label: "Jina embeddings v2 base code, int8 (768-dim, code-aware, ~160 MB)",
    aliases: &["jina", "jina-code", "jina_code", "jina_code_v2"],
    hf_base: "https://huggingface.co/jinaai/jina-embeddings-v2-base-code/resolve/main",
    files: &[
        "onnx/model_quantized.onnx",
        TOKENIZER_NAME,
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ],
};

pub static CATALOG: &[EmbedModelSpec] = &[MINILM_MULTILINGUAL_Q, JINA_CODE_V2];

#[derive(Debug, Clone, Copy, Default)]
pub struct InstallOptions {
    pub quiet: bool,
    pub force: bool,
}

#[derive(Debug)]
pub enum ModelInstallError {
    UnknownId(String),
    Io(io::Error),
    Download(String),
}

impl std::fmt::Display for ModelInstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "unknown embed model: {id}"),
            Self::Io(e) => write!(f, "{e}"),
            Self::Download(msg) => write!(f, "download failed: {msg}"),
        }
    }
}

impl std::error::Error for ModelInstallError {}

impl From<io::Error> for ModelInstallError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn parse_model_id(raw: &str) -> Option<&'static EmbedModelSpec> {
    let key = raw.trim().to_lowercase();
    CATALOG.iter().find(|spec| {
        spec.id.eq_ignore_ascii_case(&key)
            || spec.aliases.iter().any(|a| a.eq_ignore_ascii_case(&key))
    })
}

pub fn default_models_root() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neuromesh")
        .join("models")
}

pub fn model_install_dir(spec: &EmbedModelSpec) -> PathBuf {
    default_models_root().join(spec.dir_name)
}

pub fn is_model_installed(spec: &EmbedModelSpec) -> bool {
    spec_ready(spec, &model_install_dir(spec))
}

/// Local file name of a spec entry: `onnx/model_quantized.onnx` is saved
/// as `model_quantized.onnx` next to the tokenizer.
fn local_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// Every file of the spec is on disk.
fn spec_ready(spec: &EmbedModelSpec, dir: &Path) -> bool {
    spec.files.iter().all(|f| dir.join(local_name(f)).is_file())
}

pub fn list_installed() -> Vec<(EmbedModelSpec, PathBuf)> {
    CATALOG
        .iter()
        .filter_map(|spec| {
            let dir = model_install_dir(spec);
            if spec_ready(spec, &dir) {
                Some((*spec, dir))
            } else {
                None
            }
        })
        .collect()
}

pub fn install_hint() -> &'static str {
    "Run: neuromesh install embed minilm"
}

pub fn install_hint_with_flag(engine: &str) -> String {
    format!(
        "{engine} requires the MiniLM embedding model.\n\
         {hint}\n\
         Or:  neuromesh config engine {engine} --yes",
        hint = install_hint()
    )
}

#[cfg(feature = "download")]
pub fn install_model(
    spec: &EmbedModelSpec,
    opts: InstallOptions,
) -> Result<PathBuf, ModelInstallError> {
    install_model_inner(spec, opts)
}

#[cfg(not(feature = "download"))]
pub fn install_model(
    _spec: &EmbedModelSpec,
    _opts: InstallOptions,
) -> Result<PathBuf, ModelInstallError> {
    Err(ModelInstallError::Download(
        "this binary was built without embed download support; rebuild with --features embeddings"
            .into(),
    ))
}

#[cfg(feature = "download")]
fn install_model_inner(
    spec: &EmbedModelSpec,
    opts: InstallOptions,
) -> Result<PathBuf, ModelInstallError> {
    let dest = model_install_dir(spec);
    std::fs::create_dir_all(&dest)?;

    if !opts.force && spec_ready(spec, &dest) {
        if !opts.quiet {
            eprintln!("{} already installed at {}", spec.id, dest.display());
        }
        return Ok(dest);
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent("neuromesh-embed-install/1.0")
        .build()
        .map_err(|e| ModelInstallError::Download(e.to_string()))?;

    for name in spec.files {
        let local = local_name(name);
        let out = dest.join(local);
        if !opts.force && out.is_file() {
            if !opts.quiet {
                eprintln!("  skip {local} (exists)");
            }
            continue;
        }
        let url = format!("{}/{}", spec.hf_base, name);
        if !opts.quiet {
            eprintln!("  fetch {local}…");
        }
        // A slow or flaky link drops large files mid-body: retry the whole
        // file a few times before giving up (the .download temp never
        // becomes the real file unless it arrived complete).
        let mut last_err = String::new();
        let mut bytes = None;
        for attempt in 1..=3 {
            let result = client
                .get(&url)
                .send()
                .map_err(|e| e.to_string())
                .and_then(|r| {
                    if r.status().is_success() {
                        r.bytes().map_err(|e| e.to_string())
                    } else {
                        Err(format!("HTTP {}", r.status()))
                    }
                });
            match result {
                Ok(b) => {
                    bytes = Some(b);
                    break;
                }
                Err(e) => {
                    if !opts.quiet {
                        eprintln!("  {local}: attempt {attempt} failed ({e})");
                    }
                    last_err = e;
                }
            }
        }
        let bytes =
            bytes.ok_or_else(|| ModelInstallError::Download(format!("{local}: {last_err}")))?;
        let tmp = dest.join(format!(".{local}.download"));
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &out)?;
    }

    if !spec_ready(spec, &dest) {
        return Err(ModelInstallError::Download(
            "install incomplete after download".into(),
        ));
    }

    if !opts.quiet {
        eprintln!("{} installed at {}", spec.id, dest.display());
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minilm_aliases() {
        assert_eq!(parse_model_id("minilm").map(|s| s.id), Some(MINILM_DIR));
        assert_eq!(parse_model_id("mini-lm").map(|s| s.id), Some(MINILM_DIR));
        assert_eq!(
            parse_model_id("minilm-multilingual-q").map(|s| s.id),
            Some(MINILM_DIR)
        );
        assert!(parse_model_id("unknown").is_none());
    }
}
