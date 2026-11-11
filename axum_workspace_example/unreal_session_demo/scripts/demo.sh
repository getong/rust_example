#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:3000}"

for dependency in curl jq; do
  if ! command -v "$dependency" >/dev/null 2>&1; then
    printf 'Missing dependency: %s\n' "$dependency" >&2
    exit 1
  fi
done

request() {
  local expected="$1" method="$2" path="$3" token="${4:-}" body="${5:-}"
  local response status
  local arguments=(-sS --connect-timeout 3 --max-time 10 -X "$method")
  if [[ -n "$token" ]]; then
    arguments+=(-H "Authorization: Bearer $token")
  fi
  if [[ -n "$body" ]]; then
    arguments+=(-H 'Content-Type: application/json' --data "$body")
  fi
  response=$(curl "${arguments[@]}" -w $'\n%{http_code}' "$BASE_URL$path")
  status="${response##*$'\n'}"
  LAST_BODY="${response%$'\n'*}"
  printf '\n%s %s -> %s (expected %s)\n' "$method" "$path" "$status" "$expected"
  if [[ -n "$LAST_BODY" ]]; then
    printf '%s\n' "$LAST_BODY" | jq 'if type == "object" then del(.access_token, .join_ticket) else . end'
  fi
  if [[ "$status" != "$expected" ]]; then
    printf 'Unexpected HTTP status; aborting demo.\n' >&2
    exit 1
  fi
}

printf '演示服务：%s；请先启动 cargo run。\n' "$BASE_URL"
printf '\n1. JWT 只解决“你是谁”；没有凭证不能搜索。\n'
request 401 GET /sessions
request 401 POST /auth/login '' '{"username":"alice","password":"wrong"}'
request 200 POST /auth/login '' '{"username":"alice","password":"demo-password"}'
ALICE=$(printf '%s' "$LAST_BODY" | jq -er .access_token)
request 200 POST /auth/login '' '{"username":"bob","password":"demo-password"}'
BOB=$(printf '%s' "$LAST_BODY" | jq -er .access_token)
request 200 POST /auth/login '' '{"username":"charlie","password":"demo-password"}'
CHARLIE=$(printf '%s' "$LAST_BODY" | jq -er .access_token)

printf '\n2. Alice 创建并更新会话；房主占一个名额。\n'
SETTINGS='{"name":"Alice room","map":"Arena","mode":"Deathmatch","max_players":2,"advertised":true,"allow_join_in_progress":false}'
request 201 POST /sessions "$ALICE" "$SETTINGS"
SESSION_ID=$(printf '%s' "$LAST_BODY" | jq -er .session.id)
HOST_TICKET=$(printf '%s' "$LAST_BODY" | jq -er .join_ticket)
SETTINGS='{"name":"Alice room","map":"Harbor","mode":"Deathmatch","max_players":2,"advertised":true,"allow_join_in_progress":false}'
request 200 PUT "/sessions/$SESSION_ID/settings" "$ALICE" "$SETTINGS"
request 200 GET '/sessions?map=Harbor&mode=Deathmatch' "$BOB"
printf '%s' "$LAST_BODY" | jq -e --arg id "$SESSION_ID" 'any(.[]; .id == $id)' >/dev/null

printf '\n3. Bob 加入；Charlie 虽然 JWT 合法，也不能挤进满员房间。\n'
request 200 POST "/sessions/$SESSION_ID/join" "$BOB"
BOB_TICKET=$(printf '%s' "$LAST_BODY" | jq -er .join_ticket)
printf '%s' "$LAST_BODY" | jq -e '.session.members | length == 2' >/dev/null
request 200 POST "/sessions/$SESSION_ID/join" "$BOB"
printf '%s' "$LAST_BODY" | jq -e '.session.members | length == 2' >/dev/null
request 409 POST "/sessions/$SESSION_ID/join" "$CHARLIE"
request 403 POST "/sessions/$SESSION_ID/start" "$BOB"

printf '\n4. 加入不等于连接；API token 和游戏票据不能混用。\n'
request 401 GET /sessions "$BOB_TICKET"
request 401 POST /game/connect '' "$(jq -nc --arg ticket "$BOB" '{ticket:$ticket}')"
request 200 POST /game/connect '' "$(jq -nc --arg ticket "$BOB_TICKET" '{ticket:$ticket}')"

printf '\n5. 退出即撤销成员资格；重新加入也不能复活旧票据。\n'
request 200 POST "/sessions/$SESSION_ID/leave" "$BOB"
request 403 POST /game/connect '' "$(jq -nc --arg ticket "$BOB_TICKET" '{ticket:$ticket}')"
request 200 POST "/sessions/$SESSION_ID/join" "$BOB"
NEW_BOB_TICKET=$(printf '%s' "$LAST_BODY" | jq -er .join_ticket)
request 403 POST /game/connect '' "$(jq -nc --arg ticket "$BOB_TICKET" '{ticket:$ticket}')"
request 200 POST /game/connect '' "$(jq -nc --arg ticket "$NEW_BOB_TICKET" '{ticket:$ticket}')"
request 409 POST "/sessions/$SESSION_ID/leave" "$ALICE"

printf '\n6. 只有房主能开局；禁止中途加入，与身份鉴权无关。\n'
request 200 POST "/sessions/$SESSION_ID/start" "$ALICE"
request 409 POST "/sessions/$SESSION_ID/start" "$ALICE"
request 409 PUT "/sessions/$SESSION_ID/settings" "$ALICE" "$SETTINGS"
request 200 POST "/sessions/$SESSION_ID/leave" "$BOB"
request 409 POST "/sessions/$SESSION_ID/join" "$CHARLIE"

printf '\n7. End 保留会话记录，Destroy 删除记录。\n'
request 200 POST "/sessions/$SESSION_ID/end" "$ALICE"
request 200 GET "/sessions/$SESSION_ID" "$ALICE"
request 409 POST /game/connect '' "$(jq -nc --arg ticket "$HOST_TICKET" '{ticket:$ticket}')"
request 409 POST "/sessions/$SESSION_ID/join" "$BOB"
request 200 GET '/sessions?map=Harbor' "$ALICE"
printf '%s' "$LAST_BODY" | jq -e --arg id "$SESSION_ID" 'all(.[]; .id != $id)' >/dev/null
request 403 DELETE "/sessions/$SESSION_ID" "$BOB"
request 204 DELETE "/sessions/$SESSION_ID" "$ALICE"
request 404 GET "/sessions/$SESSION_ID" "$ALICE"
request 404 POST /game/connect '' "$(jq -nc --arg ticket "$HOST_TICKET" '{ticket:$ticket}')"

printf '\n完成：身份、发现、名额、成员资格、连接准入、生命周期是不同职责。\n'
