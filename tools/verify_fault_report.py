#!/usr/bin/env python3
"""Fail-closed verification and report-generation of fault-injection evidence."""
import argparse
import json
import os
from pathlib import Path
import sys

def validate(manifest, raw_lines):
    reqs = manifest["requirements"]
    by_case = {r["scenario"]: r for r in reqs}
    if len(by_case) != len(reqs) or len({r["id"] for r in reqs}) != len(reqs):
        raise ValueError("Duplicate requirement or scenario IDs in manifest")
    seen = {}
    summary = None
    for line in raw_lines:
        if not line.strip():
            continue
        record = json.loads(line)
        if record.get("type") == "summary":
            if summary is not None: raise ValueError("Duplicate summary")
            summary = record
            continue
        if record.get("type") != "scenario": raise ValueError("Unknown record type")
        sid = record.get("id")
        if sid not in by_case or sid in seen:
            raise ValueError(f"Unexpected or duplicate scenario: {sid}")
        if record.get("requirement") != by_case[sid]["id"]:
            raise ValueError(f"Invalid requirement mapping: {sid}")
        if any(not isinstance(record.get(k), str) or not record[k]
               for k in ("injection", "observed", "expected")):
            raise ValueError(f"Missing evidence: {sid}")
        if record["expected"] != record["observed"] or record.get("passed") is not True:
            raise ValueError(f"Scenario did not pass: {sid}")
        seen[sid] = record
    if set(seen) != set(by_case):
        raise ValueError(f"Missing scenarios: {sorted(set(by_case) - set(seen))}")
    if summary is None or summary.get("scenarios") != len(seen) or summary.get("failed") != 0:
        raise ValueError("Summary missing or inconsistent")
    if summary.get("method") != "deterministic-host-model" or summary.get("hardware_executed") is not False:
        raise ValueError("Misleading verification method/hardware claim")
    return seen

def report(manifest, seen, sha):
    rows = [{**r, **seen[r["scenario"]], "evidence_type":"deterministic-host-model"}
            for r in manifest["requirements"]]
    result = {"schema_version":1, "commit":sha,"passed":len(rows),"failed":0,
              "method":"deterministic-host-model","hardware_executed":False,
              "limitations":[
                  "Synthetic inputs and model timestamps; not RP2350 peripheral verification",
                  "No hardware watchdog reset or scratch-register persistence test",
                  "Deadline recovery currently counts on-time periods even at zero application work",
                  "Cortex-M33 QEMU evidence is a separate smoke check, not a full RP2350 emulation",
                  "No measured worst-case timing or ISO 26262 process certification"],
              "scenarios":rows}
    md=["# Fault-injection verification report","",
        f"**Commit:** {sha}","**Method:** deterministic software-model execution",
        f"**Status:** {len(rows)}/{len(rows)} scenarios PASS","",
        "| Requirement | Scenario | Injection | Expected | Observed | Result |",
        "|---|---|---|---|---|---|"]
    for row in rows:
        md.append(f"| {row['requirement']} | {row['scenario']} | {row['injection']} | {row['expected']} | {row['observed']} | PASS |")
    md.extend(["","## Limitations",""])
    md.extend(f"- {item}" for item in result["limitations"])
    md.append("")
    return result, "\n".join(md)

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--manifest",default="docs/verification/requirements.json")
    p.add_argument("--input",default="reports/raw-fault-campaign.jsonl")
    p.add_argument("--out",default="reports")
    p.add_argument("--sha",default=os.getenv("GITHUB_SHA","local"))
    a=p.parse_args()
    manifest=json.loads(Path(a.manifest).read_text(encoding="utf-8"))
    records=validate(manifest,Path(a.input).read_text(encoding="utf-8").splitlines())
    data,md=report(manifest,records,a.sha)
    out=Path(a.out)
    out.mkdir(parents=True,exist_ok=True)
    (out/"fault-injection-results.json").write_text(json.dumps(data,indent=2)+"\n",encoding="utf-8")
    (out/"fault-injection-report.md").write_text(md,encoding="utf-8")
    print(f"PASS: {data['passed']} traced fault scenarios")

if __name__=="__main__":
    try: main()
    except (ValueError,KeyError,OSError,json.JSONDecodeError) as e:
        print(f"VERIFICATION FAILED: {e}",file=sys.stderr)
        sys.exit(1)
