#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# install-hooks.sh – install git hooks for local development
#
# Copies (or symlinks) the project's canonical hook scripts into .git/hooks so
# every developer gets the same pre-commit checks that CI enforces.
#
# Usage:
#   ./scripts/install-hooks.sh
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

BOLD="\033[1m"
RED="\033[0;31m"
GREEN="\033[0;32m"
YELLOW="\033[0;33m"
CYAN="\033[0;36m"
RESET="\033[0m"

pass() { echo -e "${GREEN}✔${RESET}  $*"; }
fail() { echo -e "${RED}✗${RESET}  $*" >&2; }
info() { echo -e "${YELLOW}▶${RESET}  $*"; }
step() { echo -e "${CYAN}${BOLD}::${RESET}${BOLD} $*${RESET}"; }

# ── Resolve repository root ───────────────────────────────────────────────────
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$ROOT" ]]; then
    fail "Not inside a git repository. Run this script from within the project."
    exit 1
fi
cd "$ROOT"

HOOKS_DIR="$ROOT/.git/hooks"
SCRIPTS_DIR="$ROOT/scripts"

echo -e "\n${BOLD}otvi – git hook installer${RESET}\n"

# ── Sanity-check required toolchain components ────────────────────────────────
step "Checking required toolchain components"

missing=()
if ! cargo fmt --version &>/dev/null; then
    missing+=("rustfmt  →  rustup component add rustfmt")
fi
if ! cargo clippy --version &>/dev/null; then
    missing+=("clippy   →  rustup component add clippy")
fi

if [[ ${#missing[@]} -gt 0 ]]; then
    fail "Missing required components:"
    for m in "${missing[@]}"; do
        echo -e "    ${RED}•${RESET} $m"
    done
    exit 1
fi

pass "rustfmt and clippy are available"

# ── Install hooks ─────────────────────────────────────────────────────────────
step "Installing hooks into $HOOKS_DIR"

install_hook() {
    local name="$1"
    local src="$SCRIPTS_DIR/${name}.sh"
    local dst="$HOOKS_DIR/${name}"

    # If a versioned hook script exists in scripts/, symlink it so updates are
    # picked up automatically on `git pull`.  Otherwise write the hook inline.
    if [[ -f "$src" ]]; then
        ln -sf "$src" "$dst"
        chmod +x "$src"
        pass "Symlinked scripts/${name}.sh → .git/hooks/${name}"
    else
        info "No scripts/${name}.sh found – hook already installed directly"
    fi

    if [[ ! -x "$dst" ]]; then
        chmod +x "$dst"
    fi
}

# pre-commit is written directly into .git/hooks by this repo's setup; if it
# doesn't exist yet, generate it now so the script is fully self-contained.
PRE_COMMIT="$HOOKS_DIR/pre-commit"

if [[ -f "$PRE_COMMIT" ]]; then
    info "pre-commit hook already exists – skipping generation"
else
    step "Writing pre-commit hook"
    cat > "$PRE_COMMIT" << 'HOOK'
#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# pre-commit – mirrors the fmt + clippy jobs from .github/workflows/ci.yml
#
# Runs before every `git commit`. To skip in an emergency:
#   git commit --no-verify
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

BOLD="\033[1m"
RED="\033[0;31m"
GREEN="\033[0;32m"
YELLOW="\033[0;33m"
RESET="\033[0m"

pass() { echo -e "${GREEN}✔${RESET}  $*"; }
fail() { echo -e "${RED}✗${RESET}  $*"; }
info() { echo -e "${YELLOW}▶${RESET}  $*"; }

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

echo -e "\n${BOLD}Running pre-commit checks…${RESET}\n"

# ── 1. rustfmt ────────────────────────────────────────────────────────────────
info "cargo fmt --all -- --check"
if cargo fmt --all -- --check 2>&1; then
    pass "rustfmt"
else
    fail "rustfmt – run \`cargo fmt --all\` and stage the changes"
    exit 1
fi

# ── 2. clippy ─────────────────────────────────────────────────────────────────
info "cargo clippy --workspace --all-targets --all-features -- -D warnings"
if cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1; then
    pass "clippy"
else
    fail "clippy – fix the warnings above before committing"
    exit 1
fi

echo -e "\n${GREEN}${BOLD}All checks passed.${RESET}\n"
HOOK
fi

chmod +x "$PRE_COMMIT"
pass "pre-commit hook is executable"

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}Done!${RESET} The following hooks are now active:\n"
echo -e "  ${CYAN}pre-commit${RESET}  cargo fmt --all -- --check"
echo -e "             cargo clippy --workspace --all-targets --all-features -- -D warnings"
echo ""
echo -e "To bypass in an emergency:  ${BOLD}git commit --no-verify${RESET}"
echo ""
