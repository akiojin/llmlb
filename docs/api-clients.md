# API Client Examples

Sample calls for the OpenAI-compatible router with cloud prefixes.

## curl
```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openai:gpt-4o",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": false
  }'
```

## Python (requests)
```python
import requests

payload = {
    "model": "google:gemini-1.5-pro",
    "messages": [{"role": "user", "content": "Say hi in JSON"}],
    "stream": False,
}
resp = requests.post("http://localhost:8080/v1/chat/completions", json=payload)
resp.raise_for_status()
print(resp.json())
```

## Node.js (fetch)
```javascript
import fetch from "node-fetch";

const body = {
  model: "anthropic:claude-3-opus",
  messages: [{ role: "user", content: "Give me three bullets" }],
  stream: true,
};

const res = await fetch("http://localhost:8080/v1/chat/completions", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify(body),
});

for await (const chunk of res.body) {
  process.stdout.write(chunk);
}
```
