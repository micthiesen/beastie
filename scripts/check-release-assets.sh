#!/usr/bin/env bash
set -euo pipefail

bash -n packaging/macos/build-app.sh packaging/macos/build-dmg.sh \
  packaging/linux/build-appimage.sh packaging/linux/validate-layout.sh \
  steam/build.sh steam/validate-config.sh
python3 - <<'PY'
from pathlib import Path
for name in ("steam/generate-achievement-manifest.py", "steam/render-vdf.py"):
    compile(Path(name).read_text(encoding="utf-8"), name, "exec")
PY
python3 steam/generate-achievement-manifest.py steam/achievements.json /tmp/beastie-achievements-canonical.json
cmp -s steam/achievements.json /tmp/beastie-achievements-canonical.json || {
  echo 'steam/achievements.json is not canonical; regenerate it with generate-achievement-manifest.py' >&2
  exit 1
}
bash steam/validate-config.sh --mode developer --config steam/config.example.env
if bash steam/validate-config.sh --mode release --config steam/config.example.env; then
  echo 'release config placeholder test unexpectedly passed' >&2
  exit 1
fi
cargo xtask store-assets check
git diff --check
echo 'release scripts and deterministic Steam inputs verified'
