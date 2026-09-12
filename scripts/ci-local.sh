set -e
cd "C:/1/1_پروژه/5_neuromesh/repo"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
echo "== fmt";      cargo fmt --all -- --check
echo "== clippy";   cargo clippy --all-targets -- -D warnings 2>&1 | grep -E "^(error|warning)" | head -10 || true
cargo clippy --all-targets -- -D warnings > /dev/null 2>&1
echo "== test all"; cargo test --all --verbose > /dev/null 2>&1
echo "== isolation gate"; cargo test -p neuromesh-context --test cross_project_isolation -- --nocapture > /dev/null 2>&1
echo "== ml gate";  cargo test -p neuromesh-context --test ml_artifact_graph -- --nocapture > /dev/null 2>&1
echo "== notebook gate"; cargo test -p neuromesh-context --test notebook_graph -- --nocapture > /dev/null 2>&1
echo "== determinism gate"; cargo test -p neuromesh-context --test packet_determinism -- --nocapture > /dev/null 2>&1
echo "== task success gate"; cargo test -p neuromesh-context --test task_success -- --nocapture > /dev/null 2>&1
if [ -n "${NM_THIRD_PARTY:-}" ]; then echo "== third-party gold"; bash scripts/fetch-third-party.sh > /dev/null; NM_THIRD_PARTY=1 cargo test -p neuromesh-context --test third_party_gold -- --nocapture > /dev/null 2>&1; fi
echo "== embeddings"; cargo test -p neuromesh-context --features embeddings --verbose > /dev/null 2>&1
echo "== embed crate"; cargo test -p neuromesh-embed --verbose > /dev/null 2>&1
echo "== graph proxy"; cargo test -p neuromesh-graph-proxy --verbose > /dev/null 2>&1
echo "ALL CI STEPS PASSED LOCALLY"
