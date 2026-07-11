# Brick shell hook for zsh.
typeset -g BRICK_LAST_COMMAND=""
typeset -g BRICK_LAST_STARTED_AT=0
typeset -g BRICK_LAST_CWD=""

function brick_preexec() {
  BRICK_LAST_COMMAND="$1"
  BRICK_LAST_STARTED_AT=$(date +%s%3N)
  BRICK_LAST_CWD="$PWD"
}

function brick_precmd() {
  local exit_code=$?
  if [[ -z "$BRICK_LAST_COMMAND" || -z "$BRICK_SHELL_SOCKET" ]]; then
    return
  fi

  local now=$(date +%s%3N)
  local duration=$((now - BRICK_LAST_STARTED_AT))
  local branch dirty
  branch=$(git -C "$BRICK_LAST_CWD" rev-parse --abbrev-ref HEAD 2>/dev/null || true)
  if [[ -z "$branch" || "$branch" == "HEAD" ]]; then
    branch=null
  else
    branch=$(printf '%s' "$branch" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')
  fi
  if [[ -n "$(git -C "$BRICK_LAST_CWD" status --porcelain 2>/dev/null)" ]]; then
    dirty=true
  else
    dirty=false
  fi

  python3 - "$BRICK_SHELL_SOCKET" "$BRICK_LAST_COMMAND" "$BRICK_LAST_CWD" "$exit_code" "$duration" "$BRICK_LAST_STARTED_AT" "$branch" "$dirty" <<'PY'
import json, socket, sys
sock, command, cwd, exit_code, duration, started, branch_json, dirty = sys.argv[1:]
branch = None if branch_json == "null" else json.loads(branch_json)
payload = {
  "pane_id": __import__("os").environ.get("BRICK_PANE_ID", ""),
  "command": command,
  "cwd": cwd,
  "exit_code": int(exit_code),
  "duration_ms": int(duration),
  "started_at": int(started),
  "git_branch": branch,
  "git_dirty": dirty == "true",
  "shell": "zsh",
}
client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
try:
  client.connect(sock)
  client.sendall((json.dumps(payload) + "\n").encode())
finally:
  client.close()
PY

  BRICK_LAST_COMMAND=""
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec brick_preexec
add-zsh-hook precmd brick_precmd
