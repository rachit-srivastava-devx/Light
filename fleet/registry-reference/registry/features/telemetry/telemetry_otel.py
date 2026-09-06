#!/usr/bin/env python3
# telemetry_otel.py — the only code in fleet that imports phoenix/openinference/tiktoken.
# Called exclusively by registry/features/telemetry/telemetry.sh (never invoked directly by a human). One JSON object
# on stdin, one JSON object on stdout. No hand-rolled OTel plumbing: phoenix.otel.register()
# is the upstream's own wiring helper (TracerProvider + OTLP exporter), used as documented.
#
# Token usage is emitted as standard GenAI events: the numeric
# gen_ai.client.token.usage value is split by gen_ai.token.type=input|output. The fleet.*
# attributes are only correlation metadata; they are never presented as token measurements.
import json
import sys


def cmd_span(p):
    from opentelemetry.trace import Status, StatusCode

    from phoenix.otel import register

    tp = register(
        endpoint=p.get("endpoint", "http://127.0.0.1:6006") + "/v1/traces",
        project_name=p.get("project", "fleet"),
        protocol="http/protobuf",
        batch=False,
        verbose=False,
        set_global_tracer_provider=False,
    )
    tracer = tp.get_tracer("fleet.telemetry")
    dur_ns = int(p.get("duration_ms", 0)) * 1_000_000
    end = __import__("time").time_ns()
    start = end - dur_ns if dur_ns > 0 else end
    span = tracer.start_span(f"fleet.{p['event']}", start_time=start)
    exit_code = int(p.get("exit_code", 0))
    span.set_attribute("openinference.span.kind", "LLM")
    span.set_attribute("fleet.component", p["component"])
    span.set_attribute("fleet.task_id", p["task_id"])
    span.set_attribute("fleet.attempt", int(p.get("attempt", 1)))
    span.set_attribute("fleet.exit_code", exit_code)
    span.set_attribute("fleet.token_source", p.get("token_source", "unknown"))
    for k in ("generator_model", "verifier_model"):
        if p.get(k):
            span.set_attribute(f"fleet.{k}", p[k])
    if p.get("model"):
        span.set_attribute("llm.model_name", p["model"])
        span.set_attribute("llm.provider", p.get("provider", "unknown"))
        span.set_attribute("gen_ai.request.model", p["model"])
    tin, tout = p.get("tokens_in"), p.get("tokens_out")
    if tin is not None:
        span.add_event(
            "gen_ai.client.token.usage",
            attributes={"gen_ai.client.token.usage": int(tin), "gen_ai.token.type": "input"},
        )
    if tout is not None:
        span.add_event(
            "gen_ai.client.token.usage",
            attributes={"gen_ai.client.token.usage": int(tout), "gen_ai.token.type": "output"},
        )
    span.set_status(Status(StatusCode.OK if exit_code == 0 else StatusCode.ERROR))
    span.end(end_time=end)
    ctx = span.get_span_context()
    tp.force_flush(timeout_millis=5000)
    tp.shutdown()
    print(json.dumps({"trace_id": format(ctx.trace_id, "032x"), "span_id": format(ctx.span_id, "016x")}))


def cmd_tokens(p):
    import tiktoken

    try:
        enc = tiktoken.encoding_for_model(p.get("model", ""))
    except KeyError:
        enc = tiktoken.get_encoding("o200k_base")  # honest fallback, still a real tokenizer
    print(json.dumps({"count": len(enc.encode(p.get("text", ""))), "encoding": enc.name}))


def _value(row, key):
    for name in (f"attributes.fleet.{key}", f"fleet.{key}", key):
        value = row.get(name)
        if value is not None:
            return value
    attrs = row.get("attributes.fleet")
    if isinstance(attrs, dict):
        return attrs.get(key)
    return None


def _trace_id(row):
    for name in ("context.trace_id", "trace_id", "context.traceId"):
        value = row.get(name)
        if value is not None:
            return str(value)
    return None


def _load(p):
    import phoenix as px

    client = px.Client(endpoint=p.get("endpoint", "http://127.0.0.1:6006"))
    return client.get_spans_dataframe(project_name=p.get("project", "fleet"))


def cmd_readback(p):
    df = _load(p)
    rows = []
    for _, row in df.iterrows():
        if _value(row, "task_id") == p.get("task_id"):
            rows.append(_trace_id(row))
    rows = [trace for trace in rows if trace]
    if not rows:
        raise RuntimeError(f"no Phoenix span found for task_id={p.get('task_id')}")
    print(json.dumps({"trace_id": rows[-1], "span_count": len(rows)}))


def cmd_p50(p):
    df = _load(p)
    prefix = p.get("task_prefix", "")
    latencies = []
    for _, row in df.iterrows():
        task_id = _value(row, "task_id")
        if not isinstance(task_id, str) or not task_id.startswith(prefix + "-"):
            continue
        start, end = row.get("start_time"), row.get("end_time")
        if start is not None and end is not None:
            latencies.append((end - start).total_seconds() * 1000)
    if not latencies:
        raise RuntimeError(f"no Phoenix spans found for task_prefix={prefix}")
    latencies.sort()
    middle = len(latencies) // 2
    p50 = latencies[middle] if len(latencies) % 2 else (latencies[middle - 1] + latencies[middle]) / 2
    print(json.dumps({"phoenix_p50": p50, "span_count": len(latencies)}))


def cmd_export(p):
    # Flatten Phoenix's span dataframe into flat JSONL rows fleet's own fields survive intact
    # (pandas nests third-party attribute namespaces into dict-valued columns, which DuckDB's
    # read_json_auto cannot group/aggregate on directly) -- this is the one piece of glue code
    # this file owns, and it does no math, only field selection.
    df = _load(p)
    for _, row in df.iterrows():
        start, end = row.get("start_time"), row.get("end_time")
        latency_ms = int(round((end - start).total_seconds() * 1000)) if start is not None and end is not None else None
        print(
            json.dumps(
                {
                    "name": row.get("name"),
                    "status_code": row.get("status_code"),
                    "latency_ms": latency_ms,
                    "trace_id": _trace_id(row),
                    "task_id": _value(row, "task_id"),
                    "attempt": _value(row, "attempt"),
                    "component": _value(row, "component"),
                    "exit_code": _value(row, "exit_code"),
                    "token_source": _value(row, "token_source"),
                    "model": row.get("gen_ai.request.model"),
                },
                default=str,
            )
        )


CMDS = {"span": cmd_span, "tokens": cmd_tokens, "export": cmd_export, "readback": cmd_readback, "p50": cmd_p50}

if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd not in CMDS:
        print(json.dumps({"error": f"unknown command '{cmd}', want one of {sorted(CMDS)}"}), file=sys.stderr)
        sys.exit(2)
    try:
        CMDS[cmd](json.loads(sys.stdin.read() or "{}"))
    except Exception as exc:  # noqa: BLE001 -- fleet's own exit-code contract, never a stack trace on stdout
        print(json.dumps({"error": str(exc)}), file=sys.stderr)
        sys.exit(4)
