wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"
wrk.body = [[
{
  "model": "google:gemini-1.5-pro",
  "messages": [
    {"role": "user", "content": "benchmark ping"}
  ],
  "stream": false
}
]]
