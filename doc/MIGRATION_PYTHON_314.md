# Migration vers Python 3.14

## 📋 Vue d'ensemble

Ce document décrit le travail nécessaire pour supporter Python 3.14 dans `protoruf`.

---

## 🔍 État Actuel

| Élément | Version |
|---------|---------|
| **PyO3** | 0.23 |
| **Python minimum** | 3.13 |
| **Édition Rust** | 2021 |
| **Statut Python 3.14** | ❌ Non supporté |

---

## 🎯 Objectif

Supporter officiellement **Python 3.14** (stable, sorti octobre 2025) en migrant vers **PyO3 0.27+**.

---

## 📊 Analyse de Compatibilité PyO3

### Versions et Support Python 3.14

| Version PyO3 | Support 3.14 | Statut |
|--------------|--------------|--------|
| 0.23 (actuelle) | ❌ Non | Version antérieure à 3.14 |
| 0.25.0+ | ⚠️ Beta | Support initial |
| 0.26.0+ | ✅ Oui | Support complet + free-threaded |
| 0.27.0+ | ✅ Oui | Testé contre 3.14.0 final |
| 0.28.x | ✅ Oui | Support stable maintenu |

### Version Cible Recommandée

**PyO3 0.27** ou **0.28** (dernière version stable)

---

## ⚠️ Breaking Changes (0.23 → 0.27+)

### 1. API GIL - Renommage (v0.26)

```rust
// ❌ Avant (0.23)
Python::with_gil(|py| {
    // code
});

// ✅ Après (0.27+)
Python::attach(|py| {
    // code
});
```

**Impact :** `src/lib.rs` - fonctions utilisant le GIL

---

### 2. `downcast()` → `cast()` (v0.27)

```rust
// ❌ Avant
obj.downcast::<PyType>()?

// ✅ Après
obj.cast::<PyType>()?
```

**Impact :** Code utilisant les casts de types Python

---

### 3. `FromPyObject` avec Lifetime (v0.27)

```rust
// ❌ Avant
impl<'py> FromPyObject<'py> for MyType {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> { ... }
}

// ✅ Après
impl<'a, 'py> FromPyObject<'a, 'py> for MyType {
    type Error = PyErr;
    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> { ... }
}
```

**Impact :** Implémentations custom (si présentes)

---

### 4. `GILOnceCell` → `PyOnceLock` (v0.26)

```rust
// ❌ Avant
use pyo3::sync::GILOnceCell;
static CELL: GILOnceCell<Py<PyType>> = GILOnceCell::new();

// ✅ Après
use pyo3::sync::PyOnceLock;
static CELL: PyOnceLock<Py<PyType>> = PyOnceLock::new();
```

**Impact :** Variables statiques avec initialisation lazy

---

### 5. `PyObject` Déprécié (v0.26)

```rust
// ❌ Avant
PyObject

// ✅ Après
Py<PyAny>
```

**Impact :** Annotations de types utilisant `PyObject`

---

### 6. `Python::allow_threads` → `Python::detach` (v0.26)

```rust
// ❌ Avant
py.allow_threads(|| { ... })

// ✅ Après
py.detach(|| { ... })
```

**Impact :** Code libérant le GIL temporairement

---

## 📝 Tâches à Réaliser

### Phase 1 : Préparation

- [ ] Créer une branche `feat/python-3.14-support`
- [ ] Sauvegarder l'état actuel (tag git)
- [ ] Lire les release notes de PyO3 0.24-0.28

### Phase 2 : Migration du Code

- [ ] Mettre à jour `Cargo.toml` : `pyo3 = "0.27"` (ou version cible)
- [ ] Remplacer `Python::with_gil` → `Python::attach`
- [ ] Remplacer `downcast()` → `cast()`
- [ ] Remplacer `GILOnceCell` → `PyOnceLock` (si utilisé)
- [ ] Remplacer `PyObject` → `Py<PyAny>` (si utilisé)
- [ ] Mettre à jour les imports si nécessaire

### Phase 3 : Tests

- [ ] Exécuter `cargo test --lib` (tests Rust)
- [ ] Exécuter `uv run pytest` (tests Python 3.13)
- [ ] Tester avec Python 3.14 (si disponible)
- [ ] Vérifier `uv run mypy` (typage)

### Phase 4 : Documentation

- [ ] Mettre à jour `README.md` (versions supportées)
- [ ] Mettre à jour `pyproject.toml` (Python 3.13+ ou 3.14+)
- [ ] Ajouter une note dans `CHANGELOG.md`

---

## 🧪 Matrice de Tests

| Python | PyO3 0.23 | PyO3 0.27+ |
|--------|-----------|------------|
| 3.11 | ✅ | ✅ |
| 3.12 | ✅ | ✅ |
| 3.13 | ✅ | ✅ |
| 3.14 | ❌ | ✅ |
| 3.14t (free-threaded) | ❌ | ✅ |

---

## 📦 Fichiers à Modifier

| Fichier | Changements | Complexité |
|---------|-------------|------------|
| `Cargo.toml` | Version PyO3 | Faible |
| `src/lib.rs` | API GIL, casts | Moyenne |
| `src/core.rs` | Aucun | - |
| `README.md` | Documentation | Faible |
| `pyproject.toml` | Version Python | Faible |

---

## ⏱️ Effort Estimé

| Tâche | Temps |
|-------|-------|
| Migration code Rust | 1-2 heures |
| Tests et débogage | 1-2 heures |
| Documentation | 30 minutes |
| **Total** | **2-4 heures** |

---

## 🚀 Commandes de Validation

```bash
# Build Rust
cargo build --release

# Tests Rust
cargo test --lib

# Build Python
uv run maturin develop

# Tests Python
uv run pytest tests/ -v

# Type checking
uv run mypy python/protoruf/ --ignore-missing-imports

# Tout en une commande
cargo test --lib && uv run pytest && uv run mypy python/protoruf/ --ignore-missing-imports
```

---

## 📚 Ressources

- [PyO3 Migration Guide](https://pyo3.rs/v0.27.0/migration)
- [PyO3 Releases](https://github.com/PyO3/pyo3/releases)
- [Python 3.14 Release Notes](https://docs.python.org/3.14/whatsnew/3.14.html)
- [Free-threaded Python (PEP 703)](https://peps.python.org/pep-0703/)

---

## ✅ Checklist de Migration

```markdown
- [ ] Branche git créée
- [ ] Cargo.toml mis à jour (pyo3 0.27+)
- [ ] with_gil → attach
- [ ] downcast → cast
- [ ] GILOnceCell → PyOnceLock (si applicable)
- [ ] PyObject → Py<PyAny> (si applicable)
- [ ] Tests Rust passent
- [ ] Tests Python passent
- [ ] mypy valide
- [ ] README mis à jour
- [ ] pyproject.toml mis à jour
- [ ] Tag de version créé
```

---

## 📌 Notes

- Python 3.14 est sorti le **7 octobre 2025**
- PyO3 0.27+ supporte aussi le mode **free-threaded** (3.14t)
- La migration est **réversible** (garder une branche stable 0.23)
- Penser à tester sur **Linux, macOS, Windows** avant release

---

*Dernière mise à jour : mars 2026*
