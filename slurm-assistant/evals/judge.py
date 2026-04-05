import json
import re
from dataclasses import dataclass
from typing import Any, Dict, List


@dataclass
class CheckResult:
    ok: bool
    messages: List[str]


def load_trace(path: str) -> Dict[str, Any]:
    with open(path, 'r', encoding='utf-8') as f:
        return json.load(f)


def flatten_trace_commands(trace: Dict[str, Any]) -> List[str]:
    commands = []
    for item in trace.get('tool_calls', []):
        if item.get('tool') in {'exec_command', 'functions.exec_command'}:
            cmd = item.get('cmd') or item.get('parameters', {}).get('cmd')
            if cmd:
                commands.append(cmd)
    return commands


def _match_any(pattern: str, commands: List[str]) -> bool:
    return any(re.search(pattern, cmd) for cmd in commands)


def judge_trace_case(case: Dict[str, Any], trace: Dict[str, Any]) -> CheckResult:
    commands = flatten_trace_commands(trace)
    rules = case.get('trace_rules', {})
    msgs: List[str] = []
    ok = True

    for pattern in rules.get('must_contain_commands', []):
        if not _match_any(pattern, commands):
            ok = False
            msgs.append(f"missing required command pattern: {pattern}")

    for group in rules.get('must_contain_one_of_commands', []):
        if not any(_match_any(pattern, commands) for pattern in group):
            ok = False
            msgs.append(f"missing one-of command group: {' | '.join(group)}")

    for pattern in rules.get('must_not_contain_commands', []):
        if _match_any(pattern, commands):
            ok = False
            msgs.append(f"forbidden command pattern matched: {pattern}")

    return CheckResult(ok=ok, messages=msgs)


def parse_init_check(text: str) -> Dict[str, Any]:
    return json.loads(text)


def parse_connection_list(text: str) -> Dict[str, Any]:
    rows = []
    for line in text.splitlines():
        if not line.strip() or line.startswith('-') or line.startswith('Name '):
            continue
        if re.match(r'^\[', line):
            continue
        m = re.match(r'^(?P<name>\S+)\s+(?P<type>cluster|instance)\s+(?P<host>\S+)\s*(?P<status>.*)$', line.strip())
        if m:
            rows.append(m.groupdict())
    return {
        'rows': rows,
        'active_count': sum(1 for r in rows if '[ACTIVE]' in r.get('status', ''))
    }


def parse_gpu_status(text: str) -> Dict[str, Any]:
    out: Dict[str, Any] = {'available_rows': [], 'drain_rows': []}
    section = None
    for line in text.splitlines():
        s = line.rstrip()
        if s.startswith('[AVAILABLE]'):
            section = 'available'
            continue
        if s.startswith('[DRAIN]'):
            section = 'drain'
            continue
        if not s or s.startswith('-') or s.startswith('Node') or s.startswith('=') or s.startswith('汇总统计'):
            continue
        m = re.match(r'^(?P<node>\S+)\s+(?P<partition>\S+)\s+(?P<idle>\d+)\/(?P<total>\d+)\s+(?P<gpu_type>.+)$', s.strip())
        if m and section in {'available', 'drain'}:
            item = m.groupdict()
            item['idle'] = int(item['idle'])
            item['total'] = int(item['total'])
            out[f'{section}_rows'].append(item)
    m1 = re.search(r'可用节点:\s*(\d+)\s*个，共\s*(\d+)\s*张 GPU，(\d+)\s*张空闲', text)
    if m1:
        out['summary'] = {
            'available_nodes': int(m1.group(1)),
            'total_gpu': int(m1.group(2)),
            'idle_gpu': int(m1.group(3)),
        }
    return out


def parse_find_gpu(text: str) -> Dict[str, Any]:
    out: Dict[str, Any] = {'available_rows': [], 'busy_rows': [], 'drain_rows': []}
    section = None
    for line in text.splitlines():
        s = line.rstrip()
        if s.startswith('[AVAILABLE]'):
            section = 'available'
            continue
        if s.startswith('[BUSY]'):
            section = 'busy'
            continue
        if s.startswith('[DRAIN]'):
            section = 'drain'
            continue
        if not s or s.startswith('-') or s.startswith('Node') or s.startswith('=') or s.startswith('汇总统计'):
            continue
        m = re.match(r'^(?P<node>\S+)\s+(?P<partition>\S+)\s+(?P<gpu_idle>\d+)\/(?P<gpu_total>\d+)\s+(?P<cpu_idle>\d+)\/(?P<cpu_total>\d+)\s+(?P<gpu_type>.+)$', s.strip())
        if m and section in {'available', 'busy', 'drain'}:
            item = m.groupdict()
            for k in ['gpu_idle','gpu_total','cpu_idle','cpu_total']:
                item[k] = int(item[k])
            out[f'{section}_rows'].append(item)
    m1 = re.search(r'可用节点:\s*(\d+)\s*个，共\s*(\d+)\s*张 GPU', text)
    m2 = re.search(r'当前空闲:\s*(\d+)\s*张 GPU', text)
    if m1 and m2:
        out['summary'] = {
            'available_nodes': int(m1.group(1)),
            'total_gpu': int(m1.group(2)),
            'idle_gpu': int(m2.group(1)),
        }
    return out


def parse_jobs_table(text: str) -> Dict[str, Any]:
    lines = [ln.rstrip() for ln in text.splitlines() if ln.strip()]
    header_idx = None
    for idx, line in enumerate(lines):
        if 'JOBID' in line and 'PARTITION' in line and 'NAME' in line and 'USER' in line:
            header_idx = idx
            break
    rows = []
    if header_idx is not None:
        for line in lines[header_idx + 1:]:
            if set(line.strip()) == {'-'}:
                continue
            if line.startswith('ssh ') or line.startswith('[INFO]') or line.startswith('[WARN]'):
                continue
            parts = line.split()
            if len(parts) >= 8 and parts[0].isdigit():
                rows.append({
                    'job_id': parts[0],
                    'partition': parts[1],
                    'name': parts[2],
                    'user': parts[3],
                    'state': parts[4],
                })
    return {'has_header': header_idx is not None, 'rows': rows, 'raw_line_count': len(lines)}


def parse_plain_text(text: str) -> Dict[str, Any]:
    return {'text': text}


PARSERS = {
    'init_check': parse_init_check,
    'connection_list': parse_connection_list,
    'gpu_status': parse_gpu_status,
    'find_gpu': parse_find_gpu,
    'jobs_table': parse_jobs_table,
    'plain_text': parse_plain_text,
}


def assert_live_probe(case: Dict[str, Any], raw_text: str, parsed: Dict[str, Any]) -> CheckResult:
    spec = case.get('live_probe', {}).get('assert', {})
    msgs: List[str] = []
    ok = True

    for field in spec.get('fields_present', []):
        if field not in parsed:
            ok = False
            msgs.append(f'missing field: {field}')

    if 'min_rows' in spec:
        rows = parsed.get('rows', [])
        if len(rows) < spec['min_rows']:
            ok = False
            msgs.append(f'rows {len(rows)} < min_rows {spec["min_rows"]}')

    if spec.get('at_least_one_active') and parsed.get('active_count', 0) < 1:
        ok = False
        msgs.append('no active connection row found')

    if spec.get('must_have_summary') and 'summary' not in parsed:
        ok = False
        msgs.append('summary not parsed')

    for field in spec.get('summary_fields', []):
        if field not in parsed.get('summary', {}):
            ok = False
            msgs.append(f'summary missing field: {field}')

    if not spec.get('accept_empty', False):
        pass
    else:
        if not parsed.get('has_header', False) and not parsed.get('rows', []):
            ok = False
            msgs.append('jobs output has neither table header nor rows')

    for text in spec.get('must_contain_text', []):
        if text not in raw_text:
            ok = False
            msgs.append(f'missing text: {text}')

    return CheckResult(ok=ok, messages=msgs)
