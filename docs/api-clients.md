# API Client Examples

Sample calls for the OpenAI-compatible router with cloud prefixes.

## curl
```bash
curl -X POST http://localhost:32768/v1/responses \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openai:gpt-4o",
    "input": "Hello"
  }'
```

## Python (requests)
```python
import requests

payload = {
    "model": "google:gemini-1.5-pro",
    "input": "Say hi in JSON",
}
resp = requests.post("http://localhost:32768/v1/responses", json=payload)
resp.raise_for_status()
print(resp.json())
```

## Node.js (fetch)
```javascript
const body = {
  model: "anthropic:claude-3-opus",
  input: "Give me three bullets",
};

const res = await fetch("http://localhost:32768/v1/responses", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify(body),
});

const data = await res.json();
console.log(data);
```
