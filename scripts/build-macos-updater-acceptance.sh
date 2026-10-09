#!/usr/bin/env bash
# Disposable verification checkout only. No version commit, tag or release.
set -euo pipefail
output="${1:?Provide the acceptance output directory}"
mkdir -p "$output"
output="$(cd "$output" && pwd)"
workspace_manifest="$(mktemp)"
workspace_lock="$(mktemp)"
cp Cargo.toml "$workspace_manifest"
cp Cargo.lock "$workspace_lock"
trap 'cp "$workspace_manifest" Cargo.toml; cp "$workspace_lock" Cargo.lock; rm -f "$workspace_manifest" "$workspace_lock" "$output/temporary-updater.key"' EXIT
old_version="$(python3 -c 'import re; print(re.search(r"(?m)^version = \"([^\"]+)\"",open("Cargo.toml").read())[1])')"
new_version="$(python3 - "$old_version" <<'PY'
import sys
major, minor, patch = map(int,sys.argv[1].split('.'))
print(f'{major}.{minor}.{patch+1}')
PY
)"
pnpm dlx @tauri-apps/cli@2.11.3 signer generate --ci -p '' -w "$output/temporary-updater.key" >/dev/null
export TAURI_SIGNING_PRIVATE_KEY_PATH="$output/temporary-updater.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=''
python3 - "$output" "$new_version" <<'PY'
import json,sys
from pathlib import Path
output=Path(sys.argv[1])
spec=dict(version=sys.argv[2],pubkey=(output/'temporary-updater.key.pub').read_text().strip(),prerelease=False)
(output/'native-updater-acceptance.json').write_text(json.dumps(spec))
config=dict(bundle=dict(resources={str(output/'native-updater-acceptance.json'):'native-updater-acceptance.json'}))
(output/'acceptance.conf.json').write_text(json.dumps(config))
PY
# Both bundles retain the production identity and sandbox. Run only in a disposable account.
for version in "$old_version" "$new_version"; do
  python3 - "$version" <<'PY'
import re,sys
from pathlib import Path
p=Path('Cargo.toml')
p.write_text(re.sub(r'(?m)^version = "[^"]+"',f'version = "{sys.argv[1]}"',p.read_text(),count=1))
PY
  pnpm dlx @tauri-apps/cli@2.11.3 build --bundles app --features native-updater-acceptance \
    --config src-tauri/tauri.direct.conf.json --config "$output/acceptance.conf.json"
  app='target/release/bundle/macos/Speaker Volume Bridge.app'
  bash scripts/verify-macos-artifact.sh direct "$app"
  xcrun stapler validate "$app"
  spctl --assess --type execute "$app"
  mkdir -p "$output/$version"
  ditto "$app" "$output/$version/Speaker Volume Bridge.app"
done
cargo build --release -p speaker-volume-bridge-updater-artifact
# The fixture resource is a signed additive resource, not an official install offer.
python3 scripts/macos-updater-artifact.py \
  "$output/$new_version/Speaker Volume Bridge.app" "$new_version" "$output/candidate" \
  --public-key "$output/temporary-updater.key.pub"
python3 - "$output" "$new_version" <<'PY'
import json,sys,shutil
from pathlib import Path
output=Path(sys.argv[1]); candidate=output/'candidate'
archive=candidate/'speaker-volume-bridge-macos-aarch64.app.tar.gz'
shutil.copyfile(archive,candidate/'payload.app.tar.gz')
signature=Path(str(archive)+'.sig').read_text().strip()
manifest=dict(version=sys.argv[2],platforms={'darwin-aarch64':dict(url='http://127.0.0.1:8765/payload.app.tar.gz',signature=signature)})
(candidate/'manifest.json').write_text(json.dumps(manifest))
(output/'versions.json').write_text(json.dumps(dict(old=sys.argv[2].rsplit('.',1)[0]+'.'+str(int(sys.argv[2].rsplit('.',1)[1])-1),new=sys.argv[2])))
PY
# Delete temporary private signing material before retaining acceptance artifacts.
rm -f "$output/temporary-updater.key"
