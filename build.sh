set -e

cargo build --locked --release -j 2
cp target/release/eder bin/eder && sha256sum bin/eder > bin/SHA256SUMS && sha256sum --check bin/SHA256SUMS && file bin/eder && ls -l bin/eder bin/SHA256SUMS && git status --short