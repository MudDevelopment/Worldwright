"""Read-only GitHub.com Dependabot export plus local dependency context.
No dependency updates, installs, builds, dismissals or writes outside this repo.
The token is supplied through an existing environment variable or hidden prompt.
"""
from pathlib import Path
import argparse
from collections import Counter
import getpass
import json
import os
import re
import sys
import time
import tomllib
import uuid
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import Request, build_opener, HTTPRedirectHandler
import native_paths
import worldwright_local as local
from r6_support import Collector, encode, new_file, read, require, sha

REPO = 'MudDevelopment/Worldwright'
API = 'https://api.github.com'
ALERT_PATH = '/repos/' + REPO + '/dependabot/alerts'
REPO_PATH = '/repos/' + REPO
MAX_PAGE = 16 * 1024 * 1024
MAX_TOTAL = 64 * 1024 * 1024


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('GitHub API redirect refused; no token forwarded')


def checked_url(url, path):
    parsed = urlsplit(url)
    require(parsed.scheme == 'https' and parsed.netloc == 'api.github.com'
            and parsed.path == path and not parsed.fragment,
            'GitHub URL/endpoint outside export scope')
    return url


def get_json(opener, url, path, token):
    checked_url(url, path)
    request = Request(url, headers={'Authorization': 'Bearer ' + token,
        'Accept': 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28',
        'User-Agent': 'Worldwright-ReadOnly-Dependabot-Export'}, method='GET')
    try:
        with opener.open(request, timeout=45) as response:
            require(response.status == 200, 'GitHub did not return HTTP 200')
            payload = response.read(MAX_PAGE + 1)
            require(len(payload) <= MAX_PAGE, 'GitHub page exceeds 16 MiB')
            return json.loads(payload), payload, response.headers.get('Link', ''), response.headers.get('X-GitHub-Request-Id')
    except HTTPError as error:
        raise ValueError('GitHub HTTP ' + str(error.code) + '; verify repository access and Dependabot alerts read permission; no retry performed') from None


def next_page(header):
    links = [url for url, relations in re.findall(r'<([^>]+)>\s*;\s*rel="([^"]+)"', header) if 'next' in relations.split()]
    require(len(links) <= 1, 'ambiguous pagination')
    if 'rel="next"' in header:
        require(len(links) == 1, 'unparsed pagination; refusing partial success')
    return None if not links else checked_url(links[0], ALERT_PATH)


def export_alerts(collector, token):
    opener = build_opener(NoRedirect())
    repository, raw, _, request_id = get_json(opener, API + REPO_PATH, REPO_PATH, token)
    require(isinstance(repository, dict) and repository.get('full_name', '').lower() == REPO.lower(), 'wrong repository response')
    new_file(collector.output / 'github-repository.json', raw)
    collector.report['github_default_branch'] = repository.get('default_branch')
    collector.report['repository_request_id'] = request_id
    url = API + ALERT_PATH + '?state=open&per_page=100&sort=created&direction=asc'
    alerts = []; seen_numbers = set(); seen_urls = set(); total = 0
    collector.report['api_pages'] = []
    for page in range(1, 51):
        require(url not in seen_urls, 'pagination loop')
        seen_urls.add(url)
        value, raw, link, request_id = get_json(opener, url, ALERT_PATH, token)
        require(isinstance(value, list), 'expected an alerts array')
        total += len(raw); require(total <= MAX_TOTAL, 'export exceeds total bound')
        new_file(collector.output / ('alerts-page-' + str(page) + '.json'), raw)
        collector.report['api_pages'].append({'page': page, 'request_id': request_id, 'count': len(value), 'bytes': len(raw)})
        for alert in value:
            require(isinstance(alert, dict) and type(alert.get('number')) is int and alert.get('state') == 'open', 'unexpected alert record/state')
            require(alert['number'] not in seen_numbers, 'duplicate alert during pagination; recollect instead of deduplicating silently')
            require(isinstance(alert.get('security_advisory'), dict) and isinstance(alert.get('security_vulnerability'), dict) and isinstance(alert.get('dependency'), dict), 'missing advisory/dependency fields')
            seen_numbers.add(alert['number']); alerts.append(alert)
        require(len(alerts) <= 5000, 'too many alerts')
        url = next_page(link)
        if url is None: break
    else:
        raise ValueError('pagination limit reached; export incomplete')
    new_file(collector.output / 'dependabot-alerts.json', encode(alerts))
    counts = Counter()
    rows = []
    for alert in alerts:
        advisory = alert['security_advisory']; vulnerability = alert['security_vulnerability']; dependency = alert['dependency']
        severity = advisory.get('severity', 'unknown')
        counts['moderate' if severity == 'medium' else severity] += 1
        rows.append({'number': alert['number'], 'url': alert.get('html_url'),
            'ghsa_id': advisory.get('ghsa_id'), 'cve_id': advisory.get('cve_id'),
            'summary': advisory.get('summary'), 'severity': severity,
            'package': dependency.get('package'), 'manifest_path': dependency.get('manifest_path'),
            'scope': dependency.get('scope'), 'relationship': dependency.get('relationship'),
            'vulnerable_version_range': vulnerability.get('vulnerable_version_range'),
            'first_patched_version': vulnerability.get('first_patched_version'),
            'exploitability_assessment': 'pending source and configuration review'})
    new_file(collector.output / 'alert-index.json', encode(rows))
    collector.report.update(alert_export_complete=True, alert_count=len(alerts), severity_counts=dict(counts),
        matches_push_notice_count=len(alerts) == 24,
        snapshot_limit='Paginated live API view; not an atomic database snapshot or a claim that local HEAD equals the default branch')


def local_context(collector):
    for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'):
        new_file(collector.output / name, read(local.ROOT / name))
    collector.report['local_lock_sha256'] = sha(local.ROOT / 'Cargo.lock')
    collector.report['local_manifest_sha256'] = sha(local.ROOT / 'Cargo.toml')
    packages = tomllib.loads(read(local.ROOT / 'Cargo.lock').decode())['package']
    new_file(collector.output / 'locked-packages.json', encode(packages))
    collector.phase('local-head', ['git', 'rev-parse', 'HEAD'])
    collector.phase('local-branch', ['git', 'branch', '--show-current'])
    collector.phase('local-status', ['git', 'status', '--porcelain'])
    collector.phase('dependency-history', ['git', 'log', '--all', '-n', '20', '--no-ext-diff', '--format=commit %H%nDate: %aI%nSubject: %s', '-p', '--', 'Cargo.toml', 'Cargo.lock'])
    collector.report['dependency_history_limit'] = 'Last 20 local commits touching root manifest/lock; not a complete failed-upgrade or CI history'
    tools = native_paths.configure_native()
    collector.report['native_tools'] = tools
    cargo = tools['binaries']['cargo']['path']
    collector.phase('cargo-metadata', [cargo, 'metadata', '--format-version', '1', '--locked', '--offline', '--filter-platform', tools['host']], 180)
    collector.phase('server-runtime-tree', [cargo, 'tree', '--locked', '--offline', '--target', tools['host'], '-p', 'worldwright-server', '-e', 'normal,build'], 180)
    collector.phase('server-feature-tree', [cargo, 'tree', '--locked', '--offline', '--target', tools['host'], '-p', 'worldwright-server', '-e', 'features'], 180)
    require(sha(local.ROOT / 'Cargo.lock') == collector.report['local_lock_sha256'] and sha(local.ROOT / 'Cargo.toml') == collector.report['local_manifest_sha256'], 'manifest/lock changed during export')
    collector.report['local_context_complete'] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--token-env', default='GH_TOKEN', help='Name only, never the token value')
    args = parser.parse_args()
    require(re.fullmatch('[A-Za-z_][A-Za-z0-9_]*', args.token_env), 'invalid environment-variable name')
    local.local_path(Path(sys.prefix))
    local.configure_tools_environment()
    output = local.LOCAL / 'evidence' / ('dependabot-' + uuid.uuid4().hex)
    collector = Collector(output, 'dependabot-read-only-export')
    collector.report.update(repository=REPO, alert_export_complete=False, local_context_complete=False,
        security_approval=False, scope='Open Dependabot alerts and local dependency context; not a vulnerability assessment or remediation')
    print('Evidence directory: ' + str(output), flush=True)
    try:
        token = os.environ.get(args.token_env)
        if not token:
            require(sys.stdin.isatty(), 'interactive terminal or token environment variable required; never paste a token into chat')
            token = getpass.getpass('GitHub token (hidden; Dependabot alerts READ for MudDevelopment/Worldwright): ')
        require(token and len(token) <= 4096 and all(32 < ord(c) < 127 for c in token), 'invalid token input')
        export_alerts(collector, token)
        token = None
        # Do not pass export credentials to Git/Cargo context commands.
        for key in (args.token_env, 'GH_TOKEN', 'GITHUB_TOKEN'): os.environ.pop(key, None)
        print('Open alerts exported: ' + str(collector.report['alert_count']), flush=True)
        local_context(collector)
        collector.report['status'] = 'exported-awaiting-vulnerability-assessment'
    except (Exception, KeyboardInterrupt) as error:
        # Never persist exception bodies that might contain headers or credentials.
        collector.report['status'] = 'failed'
        collector.report['failure_type'] = type(error).__name__
        if isinstance(error, ValueError):
            message = str(error)
            collector.report['failure'] = message[:500]
            print(message, flush=True)
        else:
            print('Export stopped: ' + type(error).__name__ + '; preserve the evidence directory.', flush=True)
    finally:
        collector.finish()
    return 0 if collector.report['status'] == 'exported-awaiting-vulnerability-assessment' else 1


if __name__ == '__main__':
    raise SystemExit(main())
