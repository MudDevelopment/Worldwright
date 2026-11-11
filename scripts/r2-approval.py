"""Authenticate R2 provider adoption against an independently provisioned Ed25519 public key.
This tool verifies approval; it never generates keys, signs, or grants authority.
"""
from datetime import datetime, timezone
from pathlib import Path
import base64
import json
from cryptography.hazmat.primitives.serialization import load_pem_public_key
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey


def verify(report, public_key):
    if public_key.is_symlink() or not public_key.is_file() or public_key.stat().st_size > 16384:
        raise ValueError('independently provisioned approval public key required')
    key = load_pem_public_key(public_key.read_bytes())
    if not isinstance(key, Ed25519PublicKey):
        raise ValueError('approval key must be Ed25519')
    approval = report.get('adoption')
    if not isinstance(approval, dict) or approval.get('decision') != 'approved' or not approval.get('policy_reference') or not approval.get('deployment_id'):
        raise ValueError('owner adoption and policy/deployment identity missing')
    expires = datetime.fromisoformat(approval.get('expires_at', '').replace('Z', '+00:00'))
    if expires.tzinfo is None or expires <= datetime.now(timezone.utc):
        raise ValueError('owner adoption expired or clock unqualified')
    signature = base64.b64decode(report.get('approval_signature', ''), validate=True)
    unsigned = {key: value for key, value in report.items() if key != 'approval_signature'}
    message = json.dumps(unsigned, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode('utf-8')
    try:
        key.verify(signature, message)
    except Exception as error:
        raise ValueError('owner adoption signature invalid') from error
    return approval
