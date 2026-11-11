"""Bounded R3 phase capture; no shell, target redirects or invented exit evidence."""
from pathlib import Path
import hashlib
import json
import os
import queue
import re
import signal
import subprocess
import threading
import time
from urllib.parse import unquote, urlsplit
from urllib.request import HTTPRedirectHandler

MAX_CAPTURE = 16 * 1024 * 1024

class NoRedirects(HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, newurl):
        raise ValueError('R3 API redirect refused: approved target must answer directly')

def secret_values(names=()):
    """Mask configured credential values, JSON passwords and URL passwords in logs."""
    keys=set(names)|{key for key in os.environ if re.search(r'PASSWORD|TOKEN|SECRET|DATABASE_URL|CREDENTIALS',key)}
    values=set()
    for key in keys:
        value=os.environ.get(key)
        if not value: continue
        values.add(value)
        try:
            parsed=json.loads(value)
            if isinstance(parsed,dict):
                for field in ('password','token','client_secret'):
                    if isinstance(parsed.get(field),str) and parsed[field]: values.add(parsed[field])
        except (ValueError,TypeError): pass
        try:
            password=urlsplit(value).password
            if password: values.update((password,unquote(password)))
        except ValueError: pass
    return tuple(sorted(values,key=len,reverse=True))

def log_bytes(raw,sensitive=(),limit=MAX_CAPTURE):
    result=raw
    for value in sensitive:
        token=value.encode() if isinstance(value,str) else value
        if not token: continue
        result=result.replace(token,b'[REDACTED]')
        # An output cap may bisect a secret; do not persist its terminal prefix.
        for size in range(min(len(token)-1,len(result)),0,-1):
            if result.endswith(token[:size]):
                result=result[:-size]+b'[REDACTED-PARTIAL]'; break
    return result[:limit]

def record_phase(report,output,name,command,timeout,cwd=None,sensitive=(),retain=True,expected=None):
    """Persist actual bounded phase evidence before callers classify its outcome."""
    if not isinstance(name,str) or not re.fullmatch(r'[a-z0-9-]+',name):
        raise ValueError('safe phase name required')
    output=Path(output); path=output/(name+'.log')
    if not output.is_dir() or output.is_symlink() or path.exists() or path.is_symlink():
        raise ValueError('new private phase log required')
    result=collect_phase(command,timeout,cwd=cwd)
    data=log_bytes(result['stdout'],sensitive) if retain else ('private API response omitted; exit '+str(result['returncode'])+'\n').encode()
    with os.fdopen(os.open(path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600),'wb') as log: log.write(data)
    item={'name':name,'exit_code':result['returncode'],'interruption':result['interruption'],'elapsed_seconds':result['elapsed_seconds'],'log_sha256':hashlib.sha256(data).hexdigest()}
    if expected is not None: item['expected_exit_code']=expected
    report['phases'].append(item)
    return result

def _cancel(process):
    # Cancel the group even if its leader exited while a descendant holds stdout.
    if os.name=='posix':
        try: os.killpg(process.pid,signal.SIGKILL)
        except ProcessLookupError: pass
    else:
        try:
            subprocess.run(['taskkill','/PID',str(process.pid),'/T','/F'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=5,check=False)
        except (OSError,subprocess.SubprocessError): pass
        if process.poll() is None: process.kill()

def collect_phase(command,timeout,cwd=None,limit=MAX_CAPTURE):
    if isinstance(timeout,bool) or not isinstance(timeout,(int,float)) or not 0<timeout<=3600:
        raise ValueError('bounded positive phase timeout required')
    if type(limit) is not int or not 0<limit<=MAX_CAPTURE:
        raise ValueError('bounded integer phase output limit required')
    if not isinstance(command,(list,tuple)) or not command:
        raise ValueError('nonempty argument vector required; shell is never used')
    started=time.monotonic(); deadline=started+timeout
    options={'start_new_session':True} if os.name=='posix' else {'creationflags':subprocess.CREATE_NEW_PROCESS_GROUP}
    process=subprocess.Popen(command,cwd=cwd,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,bufsize=0,**options)
    chunks=queue.Queue(maxsize=4); stop=threading.Event()
    def feed():
        try:
            while not stop.is_set():
                piece=process.stdout.read(65536)
                while not stop.is_set():
                    try: chunks.put(piece,timeout=.05); break
                    except queue.Full: pass
                if not piece: break
        except OSError:
            while not stop.is_set():
                try: chunks.put(None,timeout=.05); break
                except queue.Full: pass
        finally: process.stdout.close()
    reader=threading.Thread(target=feed,name='r3-phase-output',daemon=True); reader.start()
    output=bytearray(); interruption=None; ended=False
    try:
        while not ended:
            remaining=deadline-time.monotonic()
            if remaining<=0: interruption='timeout'; break
            try: piece=chunks.get(timeout=min(.05,remaining))
            except queue.Empty: continue
            if piece is None: interruption='output-read-failure'; break
            if not piece: ended=True; break
            room=limit-len(output); output.extend(piece[:room])
            if len(piece)>room: interruption='output-limit'; break
        if interruption: _cancel(process)
        try: process.wait(timeout=max(.01,deadline-time.monotonic()) if not interruption else 5)
        except subprocess.TimeoutExpired:
            interruption=interruption or 'timeout'; _cancel(process)
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: interruption='process-cleanup-failure'
    finally:
        stop.set()
        reader.join(timeout=2)
        if reader.is_alive():
            _cancel(process); reader.join(timeout=2)
            interruption='output-cleanup-failure'
        if process.poll() is None:
            _cancel(process)
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: interruption='process-cleanup-failure'
    return {'returncode':process.returncode,'stdout':bytes(output),'interruption':interruption,'elapsed_seconds':round(time.monotonic()-started,6)}
