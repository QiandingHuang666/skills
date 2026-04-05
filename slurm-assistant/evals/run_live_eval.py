#!/usr/bin/env python3
import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from judge import PARSERS, assert_live_probe, judge_trace_case, load_trace

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'scripts' / 'slurm-cli.py'
CASES = Path(__file__).resolve().parent / 'cases.json'

DEFAULT_CLUSTER = {
    'cluster_name': '贵州大学 HPC',
    'host': '210.40.56.85',
    'port': '21563',
    'username': 'qiandingh',
}


def load_cases():
    return json.loads(CASES.read_text(encoding='utf-8'))


def run(cmd, env, timeout=60):
    proc = subprocess.run(cmd, capture_output=True, text=True, env=env, timeout=timeout)
    return {
        'cmd': cmd,
        'returncode': proc.returncode,
        'stdout': proc.stdout,
        'stderr': proc.stderr,
    }


def build_env(config_home: Path):
    env = os.environ.copy()
    env['XDG_CONFIG_HOME'] = str(config_home)
    env.setdefault('PYTHONUTF8', '1')
    return env


def ensure_uv_or_python():
    if shutil.which('uv'):
        return ['uv', 'run', 'python']
    return [sys.executable]


def ensure_remote_init(env, args):
    base = ensure_uv_or_python() + [str(SCRIPT)]
    init_cmd = base + [
        'init', '--mode', 'remote',
        '--cluster-name', args.cluster_name,
        '--host', args.host,
        '--port', str(args.port),
        '--username', args.username,
    ]
    return run(init_cmd, env, timeout=90)


def run_live_cases(args):
    cases = load_cases()
    config_home = Path(args.config_home) if args.config_home else Path(tempfile.mkdtemp(prefix='slurm-assistant-eval-'))
    config_home.mkdir(parents=True, exist_ok=True)
    env = build_env(config_home)
    base = ensure_uv_or_python() + [str(SCRIPT)]

    init_result = None
    if not args.skip_init:
        init_result = ensure_remote_init(env, args)

    results = []
    for case in cases:
        probe = case.get('live_probe')
        if not probe:
            continue
        cmd = base + probe['argv']
        try:
            result = run(cmd, env, timeout=args.timeout)
            parser = PARSERS[probe['parser']]
            parsed = parser(result['stdout'])
            check = assert_live_probe(case, result['stdout'], parsed)
            results.append({
                'name': case['name'],
                'ok': check.ok and result['returncode'] == 0,
                'returncode': result['returncode'],
                'stdout': result['stdout'],
                'stderr': result['stderr'],
                'parsed': parsed,
                'messages': check.messages,
                'cmd': cmd,
            })
        except Exception as e:
            results.append({
                'name': case['name'],
                'ok': False,
                'returncode': -1,
                'stdout': '',
                'stderr': str(e),
                'parsed': {},
                'messages': [str(e)],
                'cmd': cmd,
            })

    report = {
        'config_home': str(config_home),
        'script': str(SCRIPT),
        'init_result': init_result,
        'results': results,
        'passed': sum(1 for r in results if r['ok']),
        'total': len(results),
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report['passed'] == report['total'] else 1


def run_trace_eval(args):
    trace = load_trace(args.trace)
    cases = load_cases()
    results = []
    for case in cases:
        check = judge_trace_case(case, trace)
        results.append({'name': case['name'], 'ok': check.ok, 'messages': check.messages})
    report = {'trace': args.trace, 'passed': sum(1 for r in results if r['ok']), 'total': len(results), 'results': results}
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report['passed'] == report['total'] else 1


def main():
    p = argparse.ArgumentParser(description='slurm-assistant 专用评测器（贵州大学集群）')
    sub = p.add_subparsers(dest='cmd', required=True)

    live = sub.add_parser('live', help='对真实集群运行 slurm-cli 并校验输出解析')
    live.add_argument('--cluster-name', default=DEFAULT_CLUSTER['cluster_name'])
    live.add_argument('--host', default=DEFAULT_CLUSTER['host'])
    live.add_argument('--port', default=DEFAULT_CLUSTER['port'])
    live.add_argument('--username', default=DEFAULT_CLUSTER['username'])
    live.add_argument('--config-home', default='')
    live.add_argument('--skip-init', action='store_true')
    live.add_argument('--timeout', type=int, default=60)

    trace = sub.add_parser('trace', help='校验模型轨迹是否遵守 skill 流程')
    trace.add_argument('--trace', required=True, help='轨迹 JSON 文件')

    args = p.parse_args()
    if args.cmd == 'live':
        raise SystemExit(run_live_cases(args))
    raise SystemExit(run_trace_eval(args))


if __name__ == '__main__':
    main()
