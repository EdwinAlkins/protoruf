# Installation et Configuration

Guide complet pour installer et configurer `protoruf`.

## Prérequis

### Système

- **OS**: Linux, macOS, ou Windows
- **Python**: 3.13 ou supérieur
- **Rust**: 1.70 ou supérieur (pour le développement)

### Outils recommandés

- **uv**: Gestionnaire de paquets Python rapide
- **maturin**: Build tool pour les extensions Python Rust

---

## Installation rapide

### Avec uv (recommandé)

```bash
# Cloner le repository
git clone https://github.com/votre-org/protoruf.git
cd protoruf

# Installer les dépendances et la librairie
uv sync
uv run maturin develop
```

### Avec pip

```bash
# Installer maturin
pip install maturin

# Build et installation
maturin develop --release
```

---

## Installation détaillée

### 1. Installer Rust

#### Linux/macOS

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustup default stable
```

#### Windows

Téléchargez et exécutez [rustup-init.exe](https://rustup.rs)

#### Vérifier l'installation

```bash
rustc --version
cargo --version
```

### 2. Installer Python 3.13+

#### Avec uv (recommandé)

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
uv python install 3.13
```

#### Autre méthode

```bash
# Ubuntu/Debian
sudo apt install python3.13 python3.13-venv python3.13-dev

# macOS (Homebrew)
brew install python@3.13

# Windows
# Téléchargez depuis python.org
```

### 3. Installer uv

```bash
# Linux/macOS
curl -LsSf https://astral.sh/uv/install.sh | sh

# Windows
powershell -c "irm https://astral.sh/uv/install.ps1 | iex"
```

### 4. Configurer l'environnement

```bash
# Créer un environnement virtuel
uv venv

# Activer l'environnement
source .venv/bin/activate  # Linux/macOS
.venv\Scripts\activate    # Windows

# Installer les dépendances
uv sync

# Build la librairie Rust
uv run maturin develop
```

---

## Vérification de l'installation

```python
# Test rapide
python -c "from protoruf import compile_proto; print('✅ Installation réussie!')"
```

---

## Structure du projet

```
protoruf/
├── Cargo.toml              # Configuration Rust
├── pyproject.toml          # Configuration Python
├── src/
│   └── lib.rs              # Code Rust principal
├── python/
│   └── protoruf/           # Package Python
│       ├── __init__.py
│       ├── compiler.py
│       └── models.py
├── proto/                  # Fichiers .proto exemple
├── examples/               # Exemples d'utilisation
└── doc/                    # Documentation
```

---

## Développement

### Build manuel

```bash
# Build debug
cargo build

# Build release
cargo build --release

# Build Python extension
maturin develop

# Build Python extension (release)
maturin develop --release
```

### Tests

```bash
# Lancer les tests Python
uv run pytest

# Lancer les tests Rust
cargo test

# Lancer tous les tests
uv run pytest && cargo test
```

### Lint et format

```bash
# Rust
cargo fmt
cargo clippy

# Python
uv run ruff format
uv run ruff check
```

---

## Dépannage

### Erreur: "No module named 'protoruf._protoruf'"

La librairie Rust n'est pas compilée. Exécutez:

```bash
uv run maturin develop
```

### Erreur: "protoc not found"

`protoruf` utilise `protox` (Rust) et ne nécessite **pas** `protoc`.

### Erreur de compilation Rust

```bash
# Nettoyer et rebuild
cargo clean
uv run maturin develop
```

### Problèmes de version Python

Vérifiez que Python 3.13 est utilisé:

```bash
uv python pin 3.13
uv sync
```

---

## Configuration avancée

### Utiliser avec un virtualenv existant

```bash
# Activer votre venv
source venv/bin/activate

# Installer maturin
pip install maturin

# Build et installer
maturin develop
```

### Build wheel pour distribution

```bash
# Build wheel
maturin build --release

# Les wheels seront dans target/wheels/
```

### Cross-compilation

```bash
# Pour Linux ARM64
maturin build --release --target aarch64-unknown-linux-gnu

# Pour macOS
maturin build --release --target x86_64-apple-darwin
```

---

## Dépendances

### Python

- `pydantic>=2.0` - Validation de données
- `protobuf>=5.0` - Support Protobuf

### Rust

- `pyo3` - Bindings Python
- `prost` - Support Protobuf
- `prost-reflect` - Réflexion Protobuf
- `protox` - Compilation .proto
- `serde_json` - Sérialisation JSON

---

## Prochaines étapes

- [API Python](python.md) - Utiliser la librairie
- [API Rust](rust.md) - Comprendre l'implémentation
- [Exemples](../examples/README.md) - Cas d'usage
