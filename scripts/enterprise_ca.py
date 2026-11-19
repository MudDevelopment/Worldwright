"""Narrow CA-chain resolution and owned Linux download-trust configuration.
No implicit trust-store installation; the explicit Windows repair command owns it.
"""
from datetime import datetime,timezone
from pathlib import Path
import hashlib
import json
import os
import re
import ssl
import stat

RSTA_SHA256='93d1dbce2c583b226bbd80c6d4480ce99bf8c9de6f9bca25f2929ecc38f05741'
CA_DIR=Path('/etc/worldwright-enterprise')
HOSTS=('static.rust-lang.org','nodejs.org','index.crates.io','static.crates.io','cdn.playwright.dev')


def require(value,message):
    if not value:raise ValueError(message)


def fingerprint(cert):
    from cryptography.hazmat.primitives import serialization
    return hashlib.sha256(cert.public_bytes(serialization.Encoding.DER)).hexdigest()


def ca_constraints(cert,now):
    from cryptography import x509
    require(cert.not_valid_before_utc<=now<=cert.not_valid_after_utc,'CA outside validity period')
    try:constraints=cert.extensions.get_extension_for_class(x509.BasicConstraints).value
    except x509.ExtensionNotFound:raise ValueError('CA BasicConstraints required') from None
    require(constraints.ca,'non-CA certificate refused')
    try:require(cert.extensions.get_extension_for_class(x509.KeyUsage).value.key_cert_sign,'CA cannot sign certificates')
    except x509.ExtensionNotFound:pass
    return constraints


def resolve_chain(anchor,pool,trusted_roots,now=None):
    """Return a unique signature-checked CA path to a self-signed trusted root.

    pool maps DER SHA256 to parsed certificates from existing Windows stores.
    This preliminary resolver is followed by real TLS path/hostname validation;
    it is not a replacement for a standards-complete X.509 path validator.
    """
    now=now or datetime.now(timezone.utc);require(anchor in pool,'approved CA not found')
    require(anchor in trusted_roots,'approved CA no longer Windows server-auth ROOT trusted')
    require(0<len(pool)<=4096,'certificate pool bound')
    paths=[];attempts=0
    def visit(ids):
        nonlocal attempts
        attempts+=1;require(attempts<=512,'CA path search bound')
        current=pool[ids[-1]]
        try:
            constraints=ca_constraints(current,now)
            if constraints.path_length is not None:require(len(ids)-1<=constraints.path_length,'CA path length exceeded')
        except (ValueError,KeyError):return
        # A store entry with a different issuer is not a self-signed root.
        if current.subject==current.issuer:
            try:current.verify_directly_issued_by(current)
            except Exception:return
            if ids[-1] in trusted_roots:paths.append(ids)
            return
        if len(ids)>=8:return
        for digest,parent in pool.items():
            if digest in ids or parent.subject!=current.issuer:continue
            try:current.verify_directly_issued_by(parent)
            except Exception:continue
            visit(ids+[digest])
    visit([anchor])
    require(len(paths)==1,'expected one complete trusted CA path; found '+str(len(paths))+' (no intermediate-only fallback)')
    return [pool[digest] for digest in paths[0]]


def configure_linux_trust():
    """Only configure worker processes after the root-owned repair receipt exists."""
    if os.name=='nt':return None
    receipt=CA_DIR/'receipt.json';bundle=CA_DIR/'chain.pem'
    if not receipt.exists():return None
    for path in (CA_DIR,receipt,bundle):
        for item in (path,*path.parents):
            require(not item.is_symlink(),'linked enterprise trust configuration refused')
        info=path.stat()
        require(info.st_uid==0 and not (stat.S_IMODE(info.st_mode)&0o022),'enterprise trust must be root-owned and not group/other writable')
        require(info.st_dev==Path('/').stat().st_dev,'enterprise trust not on owned guest root disk')
    require(receipt.stat().st_size<=65536 and bundle.stat().st_size<=131072,'enterprise trust size bound')
    record=json.loads(receipt.read_text());data=bundle.read_bytes()
    require(record.get('schema_version')==1 and record.get('anchor_sha256')==RSTA_SHA256,'enterprise trust identity mismatch')
    require(hashlib.sha256(data).hexdigest()==record.get('bundle_sha256'),'enterprise CA bundle changed')
    blocks=re.findall(rb'-----BEGIN CERTIFICATE-----\s+.*?-----END CERTIFICATE-----',data,re.S)
    digests=[hashlib.sha256(ssl.PEM_cert_to_DER_cert(block.decode('ascii'))).hexdigest() for block in blocks]
    require(digests==record.get('chain_sha256') and digests and digests[0]==RSTA_SHA256,'enterprise chain differs')
    # Extend Node's built-in public CAs; do not replace them or disable validation.
    os.environ['NODE_EXTRA_CA_CERTS']=str(bundle)
    return {'anchor_sha256':RSTA_SHA256,'chain_sha256':digests,'bundle_sha256':record['bundle_sha256'],
            'node_extra_ca_certs':str(bundle),'tls_verification_disabled':False}
