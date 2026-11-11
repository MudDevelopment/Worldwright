"""Observe a real approved HTTPS provider/browser login without handling credentials.
The human performs provider login/MFA in a headed browser. This produces ONLY an
observed_not_adopted report; full provider cases and signed owner adoption are separate.
"""
from pathlib import Path
import argparse
import importlib.util
import json
import os
import sys
from urllib.parse import urlsplit
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--origin', required=True)
    parser.add_argument('--issuer', required=True)
    parser.add_argument('--deployment-id', required=True)
    parser.add_argument('--server-artifact', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--browser-executable', type=Path)
    args = parser.parse_args()
    for value in (args.origin, args.issuer):
        url = urlsplit(value)
        if url.scheme != 'https' or not url.hostname or url.username or url.password or url.hostname.endswith('.invalid'):
            raise ValueError('actual approved HTTPS origin/issuer required')
    output = args.output.resolve()
    if output.exists() or output.is_relative_to(ROOT):
        raise ValueError('new private output outside checkout required')
    os.umask(0o077)
    output.mkdir(mode=0o700)
    spec = importlib.util.spec_from_file_location('r2_verify',ROOT/'scripts/r2-verify.py')
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    report = {'schema_version':1,'kind':'approved-provider','status':'failed','reviewer':'human-adoption-pending',
        'issuer':args.issuer,'deployment_id':args.deployment_id,'cases':[], 'full_r2_closed':False}
    log = output/'approved-provider.log'
    try:
        source = module.module('r2_approved_source','r0-verify.py').source_identity()
        report.update(source_sha256=source['source_sha256'],lock_sha256=source['lock_sha256'],artifact_sha256=module.sha(args.server_artifact))
        with sync_playwright() as playwright:
            options = {'headless':False}
            if args.browser_executable: options['executable_path']=str(args.browser_executable)
            browser = playwright.chromium.launch(**options)
            context = browser.new_context()  # TLS verification stays ON for actual providers.
            page = context.new_page()
            password_fallback = []
            observed_issuer = []
            def request_seen(request):
                url = urlsplit(request.url)
                if request.method == 'POST' and url.path == '/api/v1/session': password_fallback.append(True)
                if (url.scheme,url.netloc)==(urlsplit(args.issuer).scheme,urlsplit(args.issuer).netloc): observed_issuer.append(True)
            page.on('request',request_seen)
            page.goto(args.origin)
            print('Use the browser OIDC form and approved account/MFA; do not enter credentials in chat.')
            page.wait_for_function("document.querySelector('#status').textContent.startsWith('Attached as ')",timeout=180000)
            if password_fallback or not observed_issuer or page.locator('#command-submit').is_disabled():
                raise ValueError('OIDC provider path/playable attachment not observed')
            before=page.locator('#output li').count()
            page.locator('#command').fill('look'); page.locator('#command-submit').click()
            page.wait_for_function("before => document.querySelector('#output').childElementCount > before",arg=before)
            if page.evaluate('document.cookie'):
                raise ValueError('session or other cookie exposed to JavaScript; investigate')
            page.reload()
            page.wait_for_function("document.querySelector('#status').textContent.startsWith('Attached as ')")
            page.locator('#logout').click()
            page.wait_for_function("document.querySelector('#status').textContent==='Logged out'")
            browser.close()
        if module.module('r2_approved_final','r0-verify.py').source_identity()!=source:
            raise ValueError('source changed')
        report['status']='observed_not_adopted'
        report['cases']=[{'case':'browser-playable-attachment-no-password-fallback','status':'passed'}]
        log.write_text('Actual HTTPS provider navigation, OIDC attachment, Look, reload resume and logout observed.\nNo passwords/tokens/browser storage/authorization URLs captured. Full provider matrix and signed adoption remain required.\n')
    except Exception:
        log.write_text('Approved-provider browser observation failed. No adoption granted.\n')
    report['log_sha256']=module.sha(log)
    (output/'approved-provider.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Approved provider:',report['status'],'— NOT full R2 acceptance')
    return 0 if report['status']=='observed_not_adopted' else 1

if __name__=='__main__':
    try: raise SystemExit(main())
    except (OSError,ValueError) as error:
        print('R2 NOT ADOPTED:',error,file=sys.stderr); raise SystemExit(1)
