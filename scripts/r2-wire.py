"""Bounded loopback PostgreSQL v3 proxy for disposable R2 tests, never deployment.
Only unencrypted test connections are supported. No SQL/password payloads are logged.
A fault is claimed by exactly one simple-query COMMIT; absent interception is failure.
"""
import socket
import struct
import threading

MAX_PACKET = 16 * 1024 * 1024


def exact(sock, count):
    result = bytearray()
    while len(result) < count:
        data = sock.recv(count - len(result))
        if not data:
            raise EOFError('peer closed')
        result.extend(data)
    return bytes(result)


def frame(sock):
    header = exact(sock, 5)
    length = struct.unpack('!I', header[1:])[0]
    if not 4 <= length <= MAX_PACKET:
        raise ValueError('invalid PostgreSQL packet length')
    return header + exact(sock, length - 4)


def is_commit(packet):
    return packet[:1] == b'Q' and packet[5:].rstrip(b'\0').strip().upper().rstrip(b';') == b'COMMIT'


class CommitProxy:
    def __init__(self, upstream):
        if upstream[0] not in ('127.0.0.1', 'localhost', '::1'):
            raise ValueError('proxy upstream must be loopback disposable PostgreSQL')
        self.upstream = upstream
        self.listener = socket.socket()
        self.listener.bind(('127.0.0.1', 0))
        self.listener.listen(16)
        self.listener.settimeout(.2)
        self.address = self.listener.getsockname()
        self.lock = threading.Lock()
        self.mode = None
        self.hit = threading.Event()
        self.release = threading.Event()
        self.stop = threading.Event()
        self.connections = set()
        self.threads = []
        self.errors = []
        self.acceptor = threading.Thread(target=self.accept, daemon=True)
        self.acceptor.start()

    def arm(self, mode):
        if mode not in ('before', 'after', 'lost-ack'):
            raise ValueError('unknown fault mode')
        with self.lock:
            if self.mode is not None:
                raise ValueError('fault already armed')
            self.hit.clear()
            self.release.clear()
            self.mode = mode

    def claim(self):
        with self.lock:
            mode, self.mode = self.mode, None
            return mode

    def accept(self):
        while not self.stop.is_set():
            try:
                client, _ = self.listener.accept()
            except socket.timeout:
                continue
            except OSError:
                return
            thread = threading.Thread(target=self.connection, args=(client,), daemon=True)
            self.threads.append(thread)
            thread.start()

    def connection(self, client):
        upstream = None
        try:
            upstream = socket.create_connection(self.upstream, timeout=10)
            client.settimeout(20)
            upstream.settimeout(20)
            with self.lock:
                self.connections.update((client, upstream))
            length_bytes = exact(client, 4)
            length = struct.unpack('!I', length_bytes)[0]
            if not 8 <= length <= MAX_PACKET:
                raise ValueError('invalid startup packet')
            packet = length_bytes + exact(client, length - 4)
            if packet[4:8] != struct.pack('!I', 196608):
                raise ValueError('sslmode=disable required for isolated fault proxy')
            upstream.sendall(packet)
            fault = {'mode': None}
            committed = threading.Event()

            def backend():
                try:
                    while not self.stop.is_set():
                        packet = frame(upstream)
                        mode = fault['mode']
                        if mode in ('after', 'lost-ack'):
                            if packet[:1] == b'C' and packet[5:] == b'COMMIT\0':
                                committed.set()
                                self.hit.set()
                                if mode == 'after':
                                    if not self.release.wait(15):
                                        raise TimeoutError('kill-after-commit gate timed out')
                                return
                            if packet[:1] == b'E':
                                raise ValueError('backend COMMIT failed; not accepted')
                        client.sendall(packet)
                except (EOFError, OSError):
                    pass
                except Exception as error:
                    self.errors.append(type(error).__name__ + ': ' + str(error))
                finally:
                    try:
                        client.shutdown(socket.SHUT_RDWR)
                    except OSError:
                        pass

            reader = threading.Thread(target=backend, daemon=True)
            self.threads.append(reader)
            reader.start()
            while not self.stop.is_set():
                packet = frame(client)
                if is_commit(packet):
                    mode = self.claim()
                    if mode:
                        fault['mode'] = mode
                        if mode == 'before':
                            self.hit.set()
                            if not self.release.wait(15):
                                raise TimeoutError('kill-before-commit gate timed out')
                            return
                upstream.sendall(packet)
        except (EOFError, OSError):
            pass
        except Exception as error:
            self.errors.append(type(error).__name__ + ': ' + str(error))
        finally:
            for sock in (client, upstream):
                if sock is not None:
                    with self.lock:
                        self.connections.discard(sock)
                    try:
                        sock.shutdown(socket.SHUT_RDWR)
                    except OSError:
                        pass
                    sock.close()

    def close(self):
        self.stop.set()
        self.release.set()
        self.listener.close()
        with self.lock:
            sockets = list(self.connections)
        for sock in sockets:
            try:
                sock.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        self.acceptor.join(2)
        for thread in self.threads:
            thread.join(2)
        if any(thread.is_alive() for thread in self.threads):
            raise ValueError('proxy threads did not terminate')
