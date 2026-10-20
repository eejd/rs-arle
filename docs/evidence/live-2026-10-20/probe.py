"""tool_arg probe against a running arle: the 20 seeded tool_arg cases of quantbench (seed 0, the cases of the
2026-10-20 engine run), through OpenAI chat (non-stream, stream) and Anthropic messages (non-stream, stream)."""
import json, os, sys, urllib.request
from quantbench.qualify.cases import cases_tool_arg

base, label = sys.argv[1], sys.argv[2]
from types import SimpleNamespace as NS

cases = cases_tool_arg(20, 0)

# string-typed parameters whose text reads as a number / null / boolean (rs-arle#2)
ECHO_STR = {"type": "function", "function": {
    "name": "echo_str", "description": "Record the given text values exactly.",
    "parameters": {"type": "object", "properties": {
        "zip": {"type": "string"}, "word": {"type": "string"}}, "required": ["zip", "word"]}}}
for i, (zip_, word) in enumerate([("12345", "true"), ("20240131", "null"), ("3.50", "false"),
                                  ("8675309", "True"), ("1e3", "None"), ("42", "42")]):
    cases.append(NS(id=f"strnum-{i:02d}", group="strnum", think=False, max_tokens=128, tools=[ECHO_STR],
                    messages=[{"role": "user", "content": f"Call echo_str with zip '{zip_}' and word '{word}'."}],
                    expect={"name": "echo_str", "args": {"zip": zip_, "word": word}}))

def post(path, body, stream=False):
    req = urllib.request.Request(base + path, json.dumps(body).encode(), {"content-type": "application/json"})
    r = urllib.request.urlopen(req, timeout=300)
    return r if stream else json.loads(r.read())

def oai_body(c, stream):
    b = {"model": "m", "messages": c.messages, "tools": c.tools, "tool_choice": "auto", "temperature": 0,
         "max_tokens": c.max_tokens, "stream": stream}
    if c.think is not None:
        b["chat_template_kwargs"] = {"enable_thinking": c.think}
    return b

def oai(c):
    m = post("/v1/chat/completions", oai_body(c, False))["choices"][0]["message"]
    return [(t["function"]["name"], json.loads(t["function"]["arguments"])) for t in m.get("tool_calls") or []]

def oai_stream(c):
    calls = {}
    for line in post("/v1/chat/completions", oai_body(c, True), True):
        line = line.decode().strip()
        if not line.startswith("data:") or line.endswith("[DONE]"):
            continue
        for ch in json.loads(line[5:]).get("choices", []):
            for t in ch["delta"].get("tool_calls") or []:
                e = calls.setdefault(t["index"], {"name": "", "arguments": ""})
                e["name"] += t["function"].get("name") or ""
                e["arguments"] += t["function"].get("arguments") or ""
    return [(e["name"], json.loads(e["arguments"])) for _, e in sorted(calls.items())]

def anth_body(c, stream):
    msgs = [{"role": m["role"], "content": m["content"]} for m in c.messages]
    tools = [{"name": t["function"]["name"], "description": t["function"].get("description", ""),
              "input_schema": t["function"]["parameters"]} for t in c.tools]
    return {"model": "m", "max_tokens": c.max_tokens, "messages": msgs, "tools": tools, "temperature": 0, "stream": stream}

def anth(c):
    r = post("/v1/messages", anth_body(c, False))
    return [(b["name"], b["input"]) for b in r["content"] if b["type"] == "tool_use"]

def anth_stream(c):
    blocks, cur = [], None
    ev = None
    for line in post("/v1/messages", anth_body(c, True), True):
        line = line.decode().strip()
        if line.startswith("event:"):
            ev = line[6:].strip()
        elif line.startswith("data:"):
            d = json.loads(line[5:])
            if d.get("type") == "content_block_start" and d["content_block"]["type"] == "tool_use":
                cur = {"name": d["content_block"]["name"], "json": ""}
                blocks.append(cur)
            elif d.get("type") == "content_block_delta" and d["delta"].get("type") == "input_json_delta":
                cur["json"] += d["delta"]["partial_json"]
            elif d.get("type") == "content_block_stop":
                cur = None
    return [(b["name"], json.loads(b["json"] or "{}")) for b in blocks]

paths = {"openai": oai, "openai-stream": oai_stream, "anthropic": anth, "anthropic-stream": anth_stream}
rows, tot = [], {p: [0, 0] for p in paths}
grp = {}
for c in cases:
    want = [(c.expect["name"], c.expect["args"])]
    for p, fn in paths.items():
        try:
            got = fn(c)
        except Exception as e:  # recorded, not hidden
            got = [("ERROR", repr(e))]
        ok = got == want
        tot[p][0] += ok
        tot[p][1] += 1
        g = grp.setdefault((c.group == "strnum", p), [0, 0])
        g[0] += ok
        g[1] += 1
        rows.append({"label": label, "case": c.id, "group": c.group, "path": p, "ok": ok, "got": got, "want": want})
json.dump(rows, open(os.path.join(os.environ.get("LIVE", "."), f"probe-{label}.json"), "w"), indent=1)
print(label, "ALL", {p: f"{a}/{b}" for p, (a, b) in tot.items()})
print(label, "tool_arg(20)", {p: f"{a}/{b}" for (s, p), (a, b) in grp.items() if not s})
print(label, "strnum(6)", {p: f"{a}/{b}" for (s, p), (a, b) in grp.items() if s})
for r in rows:
    if not r["ok"]:
        print("MISMATCH", r["case"], r["path"], json.dumps(r["got"])[:200])
