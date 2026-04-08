# protoruf Documentation

Documentation complète pour la bibliothèque `protoruf`.

## Table des matières

1. [Installation et Configuration](setup.md)
2. [API Python](python.md)
3. [API Rust](rust.md)
4. [Exemples d'utilisation](../examples/README.md)

## Vue d'ensemble

`protoruf` est une bibliothèque Python écrite en Rust qui permet de convertir des messages JSON en Protobuf et vice-versa.

### Fonctionnalités principales

- ⚡ **Conversion JSON ↔ Protobuf** rapide grâce à Rust
- 📦 **Compilation de fichiers .proto** intégrée avec protox (aucune dépendance externe)
- 🔒 **Validation de type** avec Pydantic
- 🎯 **Support complet** des fonctionnalités Protobuf:
  - Messages imbriqués
  - Enums (string ↔ number)
  - Champs répétés (listes)
  - Maps (dictionnaires)
  - Oneof fields
  - Valeurs par défaut

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Python Application                       │
├─────────────────────────────────────────────────────────────┤
│  compile_proto()  │  json_to_protobuf()  │  protobuf_to_json│
├─────────────────────────────────────────────────────────────┤
│              PyO3 Python Bindings (Rust)                     │
├─────────────────────────────────────────────────────────────┤
│  protox (Rust)   │  prost-reflect       │  serde_json       │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

```python
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

# 1. Compiler un fichier .proto
descriptor = compile_proto("schema.proto")

# 2. Convertir JSON → Protobuf
json_data = '{"id": "123", "name": "Example"}'
protobuf_bytes = json_to_protobuf(json_data, descriptor, message_type="package.Message")

# 3. Convertir Protobuf → JSON
result = protobuf_to_json(protobuf_bytes, descriptor, message_type="package.Message")
```

## Liens utiles

- [Documentation Python](python.md) - API complète et références
- [Documentation Rust](rust.md) - Implémentation et extension
- [Guide d'installation](setup.md) - Configuration et dépendances
- [Exemples](../examples/README.md) - Cas d'usage concrets
