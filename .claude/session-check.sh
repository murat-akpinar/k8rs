#!/usr/bin/env bash
# Who else is holding this repo? — the check the PM ran by hand on 2026-09-06,
# after a second session had written `src/` all day and a `/clear` nearly made a
# cold session adopt its work (NOTES § D249, backlog.md).
#
# `git status` cannot answer this: a dirty tree looks the same whether it is your
# own box in flight or somebody else's. A live process with this cwd can.
#
# Wired as a SessionStart hook in .claude/settings.json, so it runs on a fresh
# session and on every /clear. Prints at most three lines; silence would make it
# impossible to tell it ran from it having nothing to say, so it always prints one.
set -u

repo=$(cd "${1:-$PWD}" && pwd -P)

# Every pid between this script and init — the session that launched us, its
# shell, and the terminal above it. Anything in here is *us*, never a peer.
mine=" "
pid=$$
while [ "$pid" != "1" ] && [ -n "$pid" ]; do
  mine="$mine$pid "
  pid=$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')
done

peers=""
for candidate in $(pgrep -f 'claude-code|claude$' 2>/dev/null); do
  case "$mine" in *" $candidate "*) continue ;; esac
  [ "$(readlink "/proc/$candidate/cwd" 2>/dev/null)" = "$repo" ] || continue
  peers="$peers $candidate($(ps -o lstart= -p "$candidate" 2>/dev/null | tr -s ' '))"
done

dirty=$(cd "$repo" && git status --porcelain 2>/dev/null | wc -l)
newest=$(cd "$repo" && git status --porcelain 2>/dev/null | awk '{print $NF}' |
  while read -r file; do [ -f "$repo/$file" ] && stat -c '%Y %y %n' "$repo/$file"; done |
  sort -rn | head -1 | cut -d' ' -f2,3,5-)

if [ -n "$peers" ]; then
  echo "SESSION CHECK — another Claude session has this repo open:$peers"
  echo "  The working tree is not yours to land. Ask it what it holds, or wait."
elif [ "$dirty" -gt 0 ]; then
  echo "SESSION CHECK — no other Claude session holds this repo; $dirty path(s) uncommitted."
  echo "  Newest: $newest — yours to finish or land, and nobody else is writing it."
else
  echo "SESSION CHECK — clean tree, no other Claude session on this repo."
fi
