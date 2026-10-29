# Profile-guided optimization (PGO) for a Linux wheel, run inside maturin-action's
# manylinux container through `before-script-linux`, so it is *sourced*: it ends by
# exporting RUSTFLAGS for the `maturin build` that follows in the same shell.
#
#   source scripts/pgo-prepare.sh python3.14
#
# 1. build an instrumented wheel (-Cprofile-generate),
# 2. install it in a throwaway venv and run scripts/pgo-train.py,
# 3. merge the profiles with the llvm-profdata of the same toolchain,
# 4. export RUSTFLAGS=-Cprofile-use=... for the real build.
#
# Measured on a laptop (cp314, `--profile dist`): encoding 11-12 % faster, decoding without
# validation 10-13 %, validated decoding 3-6 % (Pydantic dominates it).

pgo_python="$1"
pgo_dir="${GITHUB_WORKSPACE:-$PWD}/pgo"
rm -rf "$pgo_dir"
mkdir -p "$pgo_dir"

echo "::group::PGO: instrumented build"
rustup component add llvm-tools-preview
RUSTFLAGS="-Cprofile-generate=$pgo_dir/raw" \
    maturin build --profile dist --locked --out "$pgo_dir/instrumented" -i "$pgo_python"
echo "::endgroup::"

echo "::group::PGO: training"
"$pgo_python" -m venv "$pgo_dir/venv"
"$pgo_dir/venv/bin/pip" install --quiet "pydantic>=2.13.4" "$pgo_dir"/instrumented/*.whl
"$pgo_dir/venv/bin/python" scripts/pgo-train.py
echo "::endgroup::"

echo "::group::PGO: merge profiles"
pgo_host="$(rustc -vV | sed -n 's/^host: //p')"
"$(rustc --print sysroot)/lib/rustlib/$pgo_host/bin/llvm-profdata" \
    merge -o "$pgo_dir/merged.profdata" "$pgo_dir/raw"
echo "::endgroup::"

export RUSTFLAGS="-Cprofile-use=$pgo_dir/merged.profdata"
echo "PGO: building with RUSTFLAGS=$RUSTFLAGS"
