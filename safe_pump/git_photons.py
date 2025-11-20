#!/usr/bin/env python3
# git_photons.py v99 — THE ABSOLUTE FINAL LOCK
# FULL FLEET: safe_pump + seed_coin + interface + mothership + meme_template + warp_core
# 100% CORRECT PATHS | SSH ONLY | FORCE LOCK | WARP CORE INCLUDED
# @archon_sol + grok — NOV 19 2025 — THIS IS THE ONE

import os
import subprocess
from datetime import datetime
from pathlib import Path

# ROOT: /var/www/html/program
ROOT = Path("/var/www/html/program")

# FULL FLEET — INCLUDING WARP CORE
REPOS = {
    "safe_pump":           {"path": ROOT / "safe_pump",           "branch": "master", "remote": "git@github.com:hamkj7hpo/safe-pump-core.git"},
    "seed_coin":           {"path": ROOT / "seed_coin",           "branch": "master", "remote": "git@github.com:hamkj7hpo/safe-pump-core.git"},
    "safe_pump_interface": {"path": ROOT / "safe_pump_interface", "branch": "master", "remote": "git@github.com:hamkj7hpo/safe-pump-core.git"},
    "mothership":          {"path": ROOT / "mothership",          "branch": "main",   "remote": "git@github.com:hamkj7hpo/safe-pump-mothership.git"},
    "meme_template":       {"path": ROOT / "meme_template",       "branch": "main",   "remote": "git@github.com:hamkj7hpo/meme_template_rust.git"},
    "warp_core":           {"path": ROOT.parent / "warp_core",    "branch": "master", "remote": "git@github.com:hamkj7hpo/warp_core.git"},
}

print("\n" + "█" * 120)
print("GIT PHOTONS v99 — THE ABSOLUTE FINAL LOCK")
print("FULL FLEET INCLUDING WARP CORE | SSH ONLY | FORCE LOCK | IMMORTAL")
print("█" * 120)

def run(cmd, cwd):
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, cwd=cwd)
    return result

hashes = {}
success_count = 0

print(f"[{datetime.now().strftime('%H:%M:%S')}] Initiating full fleet nuclear lock...\n")

for name, info in REPOS.items():
    path = info["path"]
    branch = info["branch"]
    remote = info["remote"]

    if not path.exists():
        print(f"SKIPPED → {name} (directory missing: {path})")
        continue

    os.chdir(path)

    # Force SSH remote
    run(f"git remote set-url origin {remote}", path)
    run("git fetch origin --quiet", path)

    # Stage & commit everything
    run("git add .", path)
    commit_msg = f"mothership final lock — {datetime.now().strftime('%Y-%m-%d %H:%M:%S')} — SPMP FOREVER"
    commit_result = run(f'git commit -m "{commit_msg}" --allow-empty', path)
    if "nothing to commit" in commit_result.stdout:
        print(f"NO CHANGES → {name}")
    else:
        print(f"COMMITTED → {name} | {commit_msg.split('—')[1].strip()}")

    # Check if upstream exists
    upstream_check = run("git rev-parse --abbrev-ref --symbolic-full-name @{upstream}", path)

    if upstream_check.returncode != 0:
        push_result = run(f"git push --set-upstream origin {branch} --force", path)
        print(f"FIRST CONTACT → {name} → upstream set + force pushed")
    else:
        push_result = run("git push --force --quiet", path)
        print(f"FORCE LOCK → {name}")

    if push_result.returncode == 0:
        success_count += 1
        full_hash = run("git rev-parse HEAD", path).stdout.strip()
        short_hash = full_hash[:8]
        hashes[name] = (full_hash, short_hash)
        print(f"SUCCESS → {name} locked at {short_hash}")
    else:
        print(f"FAILED → {name} | {push_result.stderr.strip()}")

print("\n" + "█" * 120)
print("FULL FLEET LOCKDOWN COMPLETE")
print(f"SUCCESSFUL REPOS: {success_count}/{len(REPOS)}")
print("LAUNCH CERTIFICATE — SAVE THIS FOREVER")
print("█" * 120)

for name, (full, short) in hashes.items():
    print(f"{name.ljust(22)} → {full}  ({short})")

print("\n" + "█" * 120)
if success_count == len(REPOS):
    print("PERFECTION ACHIEVED")
    print("ALL REPOS LOCKED VIA SSH")
    print("UPSTREAM BRANCHES SET")
    print("FLEET IS NOW IMMORTAL")
    print("WARP CORE INCLUDED")
    print("THE VAULT IS SEALED")
    print("SPMP LAUNCH WINDOW: OPEN")
    print("ARCHON — FIRE AT WILL.")
else:
    print("WARNING: Some repos failed. Re-run after fixing SSH keys.")
print("73")
print("SPMP FOREVER.")
print("█" * 120)
