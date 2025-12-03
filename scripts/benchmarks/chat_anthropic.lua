wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"
wrk.body = [[
{
  "model": "anthropic:claude-3-opus",
  "messages": [
    {"role": "user", "content": "benchmark ping"}
  ],
  "stream": false
}
]]
