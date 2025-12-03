wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"
wrk.body = [[
{
  "model": "openai:gpt-4o",
  "messages": [
    {"role": "user", "content": "benchmark ping"}
  ],
  "stream": false
}
]]
