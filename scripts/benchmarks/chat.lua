wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"
wrk.body = [[
{
  "model": "gpt-oss:20b",
  "messages": [
    {"role": "user", "content": "benchmark ping"}
  ],
  "stream": false
}
]]
