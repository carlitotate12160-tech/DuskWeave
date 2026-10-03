import argparse
import json
import os
import subprocess
import sys
import hashlib
from typing import Dict, Any, List, Set

REVIEWED_RUN_SHA256 = "558499af2367afeb051d01e4c31e82e20087d491fee936c1fa83549bd1bd6d7a"

def compute_body_digest(filepath: str, start_line: int, end_line: int) -> str:
    with open(filepath, 'r', encoding='utf-8') as f:
        lines = f.read().splitlines()
    span_lines = lines[start_line - 1:end_line]
    content = "\n".join(span_lines) + "\n"
    return hashlib.sha256(content.encode('utf-8')).hexdigest()

def process_node(node: Dict[str, Any], filepath: str, parent_path: str, sibling_idx: int, results: List[Dict[str, Any]], all_identities: Set[str]):
    kind = node.get("kind", "")
    node_name = node.get("name", "")
    
    # "use ancestry and sibling position for report identity"
    # To ensure distinct closures don't collapse.
    identity_name = f"{node_name}@{sibling_idx}"
    current_path = f"{parent_path}::{identity_name}" if parent_path else identity_name

    if kind == "function":
        try:
            cyclomatic = node["metrics"]["cyclomatic"]["sum"]
            if not isinstance(cyclomatic, (int, float)) or isinstance(cyclomatic, bool):
                raise ValueError("metrics.cyclomatic.sum must be a number")
            
            own = float(cyclomatic)
            if not own.is_integer():
                raise ValueError("metrics.cyclomatic.sum must be integral")
            own = int(own)
            
            for child in node.get("spaces", []):
                child_cyc = child["metrics"]["cyclomatic"]["sum"]
                if not isinstance(child_cyc, (int, float)) or isinstance(child_cyc, bool):
                    raise ValueError("child cyclomatic sum must be a number")
                child_cyc_f = float(child_cyc)
                if not child_cyc_f.is_integer():
                    raise ValueError("child cyclomatic sum must be integral")
                own -= int(child_cyc_f)
                
        except KeyError:
            print("Malformed metric: missing metrics.cyclomatic.sum", file=sys.stderr)
            sys.exit(2)
        except ValueError as e:
            print(f"Malformed metric: {e}", file=sys.stderr)
            sys.exit(2)

        start_line = node.get("start_line")
        end_line = node.get("end_line")
        
        if not isinstance(start_line, int) or not isinstance(end_line, int) or start_line < 1 or end_line < start_line:
            print(f"Invalid span: start={start_line}, end={end_line}", file=sys.stderr)
            sys.exit(2)
            
        if own < 1: # "nonnegative child subtraction and own >=1 for functions."
            # Wait, nonnegative child subtraction means we cannot subtract more than the parent has?
            # Or does it mean child_cyc >= 0? The subtraction is just own = parent - sum(child).
            # And own >= 1.
            print(f"Invalid own metric < 1: {own}", file=sys.stderr)
            sys.exit(2)

        cap = 7
        digest = compute_body_digest(filepath, start_line, end_line)
        norm_file = filepath.replace('\\', '/')
        
        if norm_file.endswith("src/main.rs") and node_name == "run":
            if digest == REVIEWED_RUN_SHA256:
                cap = 10
            else:
                print(f"Warning: src/main.rs::run digest changed. Expected {REVIEWED_RUN_SHA256}, got {digest}")

        results.append({
            "filepath": norm_file,
            "path": current_path,
            "start_line": start_line,
            "end_line": end_line,
            "own": own,
            "cap": cap,
            "digest": digest
        })
        
        all_identities.add(f"{norm_file}:{current_path}")

    # Process children
    for idx, child in enumerate(node.get("spaces", [])):
        process_node(child, filepath, current_path, idx, results, all_identities)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--analyzer", required=True)
    parser.add_argument("--report-dir", required=True)
    args = parser.parse_args()

    # Discover src/**/*.rs
    src_files = []
    for root, _, files in os.walk("src"):
        for f in files:
            if f.endswith(".rs"):
                src_files.append(os.path.join(root, f))

    if not src_files:
        print("No source files found.")
        return

    # Run analyzer
    raw_dir = os.path.join(args.report_dir, "raw")
    os.makedirs(raw_dir, exist_ok=True)
    
    cmd = [args.analyzer, "-p", "src", "-m", "-O", "json", "-o", raw_dir, "-j", "1", "-w"]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            print(f"Analyzer failed with exit code {result.returncode}")
            sys.exit(2)
    except Exception as e:
        print(f"Analyzer execution failed: {e}")
        sys.exit(2)

    all_results = []
    all_identities = set()
    
    # Track expected reports vs found
    found_reports = set()
    for root, _, files in os.walk(raw_dir):
        for f in files:
            if f.endswith(".json"):
                found_reports.add(os.path.join(root, f).replace('\\', '/'))
                
    expected_reports = set()
    for filepath in src_files:
        norm = filepath.replace('\\', '/')
        # rca outputs to raw_dir / src / ... .json
        expected_report = os.path.join(raw_dir, norm + ".json").replace('\\', '/')
        expected_reports.add(expected_report)
        
        if expected_report not in found_reports:
            print(f"Missing report for {filepath}")
            sys.exit(2)
            
    if len(expected_reports) != len(found_reports):
        print(f"Unexpected or duplicate reports found. Expected {len(expected_reports)}, found {len(found_reports)}.")
        sys.exit(2)

    total_nom_functions = 0
    total_nom_closures = 0
            
    for filepath in src_files:
        norm = filepath.replace('\\', '/')
        report_path = os.path.join(raw_dir, norm + ".json").replace('\\', '/')
        try:
            with open(report_path, 'r', encoding='utf-8') as f:
                data = json.load(f)
                
            # Validate JSON object and no duplicate keys (json.load handles this implicitly in standard python, but to strictly reject duplicate keys we can use object_pairs_hook)
        except json.JSONDecodeError:
            print(f"Invalid JSON in {report_path}")
            sys.exit(2)
            
        def dict_raise_on_duplicates(ordered_pairs):
            d = {}
            for k, v in ordered_pairs:
                if k in d:
                    raise ValueError(f"Duplicate key: {k}")
                d[k] = v
            return d

        try:
            with open(report_path, 'r', encoding='utf-8') as f:
                data = json.load(f, object_pairs_hook=dict_raise_on_duplicates)
        except ValueError as e:
            print(f"Invalid JSON (duplicate keys): {e}")
            sys.exit(2)
            
        # check nom.total
        nom_total = data.get("metrics", {}).get("nom", {}).get("total", 0)
        
        # start process
        count_before = len(all_results)
        process_node(data, filepath, "", 0, all_results, all_identities)
        count_after = len(all_results)
        
        # Validate recursively counted functions/closures against the root's nom.total.
        # Wait, the root's nom.total might include trait methods, impl blocks etc.
        # nom.total is functions + closures.
        # Our process_node extracts all kind == "function", which includes methods and anonymous closures.
        # "Validate recursively counted functions/closures against the root's nom.total."
        functions_found = count_after - count_before
        if int(nom_total) != functions_found:
            print(f"Mismatch in function count for {filepath}: nom.total={nom_total}, found={functions_found}")
            sys.exit(2)

    violations = []
    for res in all_results:
        own = res["own"]
        cap = res["cap"]
        span = res["end_line"] - res["start_line"] + 1
        
        print(f"{res['filepath']} {res['path']} (lines {res['start_line']}-{res['end_line']}): own={own} cap={cap}")
        
        if span > 50:
            print(f"REVIEW_TRIGGER: {res['filepath']} {res['path']} length {span} > 50")
            
        if own > cap:
            violations.append(res)
            
    summary = {
        "total_functions": len(all_results),
        "violations": len(violations),
        "details": violations
    }
    
    with open(os.path.join(args.report_dir, "summary.json"), 'w', encoding='utf-8') as f:
        json.dump(summary, f, indent=2)
        
    with open(os.path.join(args.report_dir, "summary.txt"), 'w', encoding='utf-8') as f:
        f.write(f"Total functions: {len(all_results)}\nViolations: {len(violations)}\n")
        
    if violations:
        print(f"Failed: {len(violations)} complexity violations.")
        sys.exit(1)
        
    print("Success: 0 complexity violations.")
    sys.exit(0)

if __name__ == "__main__":
    main()
