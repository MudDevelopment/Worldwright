"""Linux launcher/diagnostic contracts; optional real Studio/CLI startup gate.

Python 3.10+, stdlib only. No production files or credentials are used. Synthetic
executables do NOT prove a Rust build; --real-studio/--real-preview additionally
exercise already-built native binaries through the same launcher and HTTP API.
"""
from __future__ import annotations
import argparse
import json
import os
import re
import shutil
import signal
import subprocess
import tempfile
import time
import unittest
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REAL_STUDIO: Path | None = None
REAL_PREVIEW: Path | None = None
HOST = "x86_64-unknown-linux-gnu"

class Fixture:
    def __init__(self, real: bool = False):
        self.temp = tempfile.TemporaryDirectory(prefix="worldwright desktop acceptance ")
        self.root = Path(self.temp.name)
        self.repo = self.root / "repository with spaces"
        scripts = self.repo / "scripts"
        scripts.mkdir(parents=True)
        for name in ("desktop.sh", "dev.sh"):
            shutil.copy2(ROOT / "scripts" / name, scripts / name)
        shutil.copy2(ROOT / "rust-toolchain.toml", self.repo / "rust-toolchain.toml")
        shutil.copy2(ROOT / "Cargo.lock", self.repo / "Cargo.lock")
        self.bin = self.root / "tools"
        self.bin.mkdir()
        self.home = self.root / "fresh home"
        self.home.mkdir()
        self.tmp = self.root / "scratch"
        self.tmp.mkdir()
        self.env = os.environ.copy()
        self.env.update(HOME=str(self.home), TMPDIR=str(self.tmp), CARGO_HOME=str(self.home / ".cargo"),
                        PATH=str(self.bin) + os.pathsep + self.env["PATH"],
                        WW_TRACE=str(self.root / "cargo trace.jsonl"), WW_CHILD=str(self.root / "child.json"))
        self.env.pop("XDG_DATA_HOME", None)
        self.env.pop("DISPLAY", None)
        self.env.pop("WAYLAND_DISPLAY", None)
        self.tool("rustc", "#!/usr/bin/env python3\nimport os,sys\nif '--print' in sys.argv:\n if os.environ.get('WW_FAIL_SYSROOT'): sys.exit(7)\n print('/synthetic/sysroot')\nelse: print('rustc 1.85.1\\nhost: x86_64-unknown-linux-gnu')\n")
        self.tool("rustup", "#!/usr/bin/env python3\nprint('synthetic toolchain/target information')\n")
        self.tool("cargo", "#!/usr/bin/env python3\nimport os,sys,json\nwith open(os.environ['WW_TRACE'],'a') as f: f.write(json.dumps(sys.argv[1:])+'\\n')\nif os.environ.get('WW_FAIL_CARGO')==sys.argv[1]: sys.exit(7)\nif '--version' in sys.argv: print('cargo 1.85.1')\n")
        outputs = self.repo / "target" / HOST / "debug"
        outputs.mkdir(parents=True)
        if real:
            if REAL_STUDIO is None or REAL_PREVIEW is None:
                raise RuntimeError("real binaries must be supplied explicitly")
            shutil.copy2(REAL_STUDIO, outputs / "worldwright-studio")
            shutil.copy2(REAL_PREVIEW, outputs / "worldwright")
        else:
            self.executable(outputs / "worldwright", "#!/usr/bin/env python3\n")
            self.executable(outputs / "worldwright-studio", "#!/usr/bin/env python3\nimport os,sys,json,signal,time\nfrom pathlib import Path\nworkspace=Path(sys.argv[sys.argv.index('--workspace')+1])\nassert workspace.parent.is_dir()\nworkspace.mkdir(exist_ok=True)\n(workspace/'.synthetic-marker').touch()\nPath(os.environ['WW_CHILD']).write_text(json.dumps({'pid':os.getpid(),'workspace':str(workspace)}))\ndef stop(signum,frame): sys.exit(0)\nsignal.signal(signal.SIGTERM,stop)\nprint('Worldwright Studio: http://127.0.0.1:43127/',flush=True)\nwhile True: time.sleep(0.05)\n")
        self.real = real
        self.process: subprocess.Popen | None = None
        self.log_handle = None
    def executable(self, path: Path, source: str) -> None:
        path.write_text(source)
        path.chmod(0o700)
    def tool(self, name: str, source: str) -> None:
        self.executable(self.bin / name, source)
    def workspace(self) -> Path:
        data = Path(self.env.get("XDG_DATA_HOME", str(self.home / ".local/share")))
        return data / "worldwright/studio"
    def launch(self, *args: str) -> str:
        log = self.root / "launcher.log"
        self.log_handle = log.open("w")
        self.process = subprocess.Popen(["bash", str(self.repo / "scripts/desktop.sh"), "--no-browser", *args],
            cwd=self.root, env=self.env, stdout=self.log_handle, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            text = log.read_text()
            if self.process.poll() is not None:
                raise AssertionError(f"launcher exited {self.process.returncode}: {text}")
            match = re.search(r"^Studio: (http://127\.0\.0\.1:\d+/)$", text, re.M)
            if match:
                if not self.real:
                    return match[1]
                try:
                    self.get_json(match[1], "api/v1/workspace")
                    return match[1]
                except (OSError, ValueError):
                    pass
            time.sleep(0.05)
        raise AssertionError(f"launcher readiness timeout: {log.read_text()}")
    def get_json(self, url: str, path: str) -> dict | list:
        with urllib.request.urlopen(url + path, timeout=3) as response:
            return json.load(response)
    def post(self, url: str, path: str, value: dict):
        request = urllib.request.Request(url + path, data=json.dumps(value).encode(), method="POST",
            headers={"Origin": url.rstrip("/"), "Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(request, timeout=3) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, json.load(error)
    def stop(self) -> None:
        if self.process is None:
            return
        child_pids = []
        proc = Path(f"/proc/{self.process.pid}/task/{self.process.pid}/children")
        if proc.exists():
            child_pids = [int(value) for value in proc.read_text().split()]
        self.process.send_signal(signal.SIGTERM)
        status = self.process.wait(timeout=13)
        if status != 143:
            raise AssertionError(f"launcher SIGTERM exit was {status}")
        for pid in child_pids:
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                continue
            raise AssertionError(f"child process {pid} survived launcher shutdown")
        self.process = None
        self.log_handle.close()
        self.log_handle = None
    def run(self, script: str, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run(["bash", str(self.repo / "scripts" / script), *args], cwd=self.root,
            env=self.env, capture_output=True, text=True, timeout=20)
    def close(self) -> None:
        try:
            if self.process is not None:
                try:
                    self.stop()
                finally:
                    if self.process is not None and self.process.poll() is None:
                        self.process.kill()
                        self.process.wait(timeout=5)
            if self.log_handle is not None:
                self.log_handle.close()
        finally:
            self.temp.cleanup()

class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.fixture = Fixture()
        self.addCleanup(self.fixture.close)
    def test_fresh_default_parent_private_native_override_and_reopen(self):
        f = self.fixture
        f.env["CARGO_TARGET_DIR"] = str(f.root / "unrelated target")
        f.env["CARGO_BUILD_TARGET"] = "x86_64-pc-windows-msvc"
        f.launch()
        workspace = f.workspace()
        self.assertTrue(workspace.is_dir())
        self.assertEqual(workspace.parent.stat().st_mode & 0o777, 0o700)
        calls = [json.loads(line) for line in Path(f.env["WW_TRACE"]).read_text().splitlines()]
        build = next(call for call in calls if call[0] == "build")
        self.assertEqual(build[build.index("--target") + 1], HOST)
        self.assertEqual(build[build.index("--target-dir") + 1], str(f.repo / "target"))
        f.stop()
        f.launch()
        self.assertTrue((workspace / ".synthetic-marker").exists())
        f.stop()
    def test_native_check_all_build_phases_use_the_explicit_host_and_target(self):
        f = self.fixture
        f.env["CARGO_BUILD_TARGET"] = "x86_64-pc-windows-msvc"
        f.env["CARGO_TARGET_DIR"] = str(f.root / "unrelated target")
        result = f.run("dev.sh", "check-native")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("PASS: native checks completed", result.stdout)
        calls = [json.loads(line) for line in Path(f.env["WW_TRACE"]).read_text().splitlines()]
        phases = [call for call in calls if call[0] in ("check", "clippy", "test", "build", "doc")]
        self.assertEqual([call[0] for call in phases], ["check", "clippy", "test", "build", "doc"])
        targets = set()
        for call in phases:
            self.assertIn("--locked", call)
            self.assertIn("--offline", call)
            self.assertEqual(call[call.index("--target") + 1], HOST)
            targets.add(call[call.index("--target-dir") + 1])
        self.assertEqual(len(targets), 1)
        self.assertTrue(Path(next(iter(targets))).is_dir())
        self.assertNotIn(str(f.root / "unrelated target"), targets)

    def test_existing_nonprivate_parent_is_not_silently_chmodded(self):
        f = self.fixture
        parent = f.workspace().parent
        parent.mkdir(parents=True)
        parent.chmod(0o755)
        result = f.run("desktop.sh", "--no-browser")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must be private", result.stderr)
        self.assertEqual(parent.stat().st_mode & 0o777, 0o755)
        self.assertFalse(f.workspace().exists())

    def test_fresh_xdg_with_spaces(self):
        f = self.fixture
        f.env["XDG_DATA_HOME"] = str(f.root / "fresh XDG data")
        f.launch()
        self.assertTrue(f.workspace().is_dir())
        f.stop()
    def test_custom_missing_parent_is_not_created(self):
        f = self.fixture
        result = f.run("desktop.sh", "--no-browser", str(f.root / "absent/studio"))
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((f.root / "absent").exists())
        self.assertIn("parent must exist", result.stderr)
    def test_symlink_and_dangling_symlink_rejected_without_following(self):
        f = self.fixture
        target = f.root / "destination"
        target.mkdir()
        link = f.home / ".local"
        link.symlink_to(target, target_is_directory=True)
        result = f.run("desktop.sh", "--no-browser")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(list(target.iterdir()), [])
        link.unlink()
        link.symlink_to(f.root / "missing-target", target_is_directory=True)
        self.assertNotEqual(f.run("desktop.sh", "--no-browser").returncode, 0)
        self.assertFalse((f.root / "missing-target").exists())
    def test_permission_denial_and_non_directory_are_explicit(self):
        f = self.fixture
        denied = f.root / "denied"
        denied.mkdir()
        denied.chmod(0)
        try:
            self.assertNotEqual(f.run("desktop.sh", "--no-browser", str(denied / "studio")).returncode, 0)
        finally:
            denied.chmod(0o700)
        blocker = f.home / ".local"
        blocker.write_text("do not overwrite")
        self.assertNotEqual(f.run("desktop.sh", "--no-browser").returncode, 0)
        self.assertEqual(blocker.read_text(), "do not overwrite")
    def test_doctor_redacts_values_and_native_check_stops_at_first_failure(self):
        f = self.fixture
        f.env["RUSTFLAGS"] = "SENSITIVE_SENTINEL"
        report = f.run("dev.sh", "doctor")
        self.assertEqual(report.returncode, 0, report.stderr)
        self.assertIn("host:", report.stdout)
        self.assertIn("Sysroot:", report.stdout)
        self.assertIn("Override RUSTFLAGS: SET (value redacted)", report.stdout)
        self.assertNotIn("SENSITIVE_SENTINEL", report.stdout)
        Path(f.env["WW_TRACE"]).unlink()
        f.env["WW_FAIL_CARGO"] = "fmt"
        failed = f.run("dev.sh", "check-native")
        self.assertEqual(failed.returncode, 7)
        calls = [json.loads(line) for line in Path(f.env["WW_TRACE"]).read_text().splitlines()]
        self.assertFalse(any(call[0] in ("check", "clippy", "test", "build", "doc") for call in calls))
        self.assertNotIn("PASS: native", failed.stdout)
        f.env.pop("WW_FAIL_CARGO")
        f.env["WW_FAIL_SYSROOT"] = "1"
        self.assertEqual(f.run("dev.sh", "doctor").returncode, 7)

class RealStudioTests(unittest.TestCase):
    def test_real_default_start_package_contract_shutdown_and_reopen(self):
        self.check_real_workspace(False)

    def test_real_fresh_xdg_start_package_shutdown_and_reopen(self):
        self.check_real_workspace(True)

    def check_real_workspace(self, xdg: bool):
        if REAL_STUDIO is None:
            self.skipTest("real-binary acceptance explicitly not requested; synthetic tests only")
        f = Fixture(real=True)
        self.addCleanup(f.close)
        if xdg:
            f.env["XDG_DATA_HOME"] = str(f.root / "real fresh XDG data")
        url = f.launch()
        snapshot = f.get_json(url, "api/v1/workspace")
        self.assertEqual(snapshot["revision"], 1)
        request = {"revision": snapshot["revision"], "snapshot_sha256": snapshot["snapshot_sha256"],
                   "release_id": "p0-expedition", "builder_identity": "p0-test", "game": "unknown"}
        status, _ = f.post(url, "api/v1/package", request)
        self.assertEqual(status, 422)
        self.assertEqual(f.get_json(url, "api/v1/artifacts"), [])
        for game in ("reference", "expedition"):
            request.update(game=game, release_id="p0-" + game)
            status, record = f.post(url, "api/v1/package", request)
            self.assertEqual(status, 200, record)
            artifact = f.workspace() / "artifacts" / (record["artifact_digest"] + ".wwr")
            inspect = subprocess.run([str(REAL_PREVIEW), "content", "inspect", str(artifact), "--json"],
                capture_output=True, text=True, timeout=10, check=True)
            self.assertIn("worldwright-" + game, json.dumps(json.loads(inspect.stdout)))
        f.stop()
        reopened = f.launch()
        self.assertEqual(f.get_json(reopened, "api/v1/workspace")["snapshot_sha256"], snapshot["snapshot_sha256"])
        self.assertEqual(len(f.get_json(reopened, "api/v1/artifacts")), 2)
        f.stop()

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--real-studio", type=Path)
    parser.add_argument("--real-preview", type=Path)
    args, tests = parser.parse_known_args()
    if os.name != "posix" or not Path("/proc").exists() or os.geteuid() == 0:
        raise SystemExit("Linux/non-root required for launcher acceptance")
    if bool(args.real_studio) != bool(args.real_preview):
        raise SystemExit("--real-studio and --real-preview must be paired")
    REAL_STUDIO = args.real_studio.resolve(strict=True) if args.real_studio else None
    REAL_PREVIEW = args.real_preview.resolve(strict=True) if args.real_preview else None
    unittest.main(argv=[__file__, *tests])
