"""Disposable-process integration regressions; no application or external endpoints."""
import ctypes
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

MODULE = Path(__file__).parents[1] / 'measure-archive.py'
DRIVER = r'''
import importlib.util,json,os,subprocess,sys,time
from pathlib import Path
module,folder,mode=sys.argv[1:]
folder=Path(folder)
spec=importlib.util.spec_from_file_location('measure',module)
measure=importlib.util.module_from_spec(spec);spec.loader.exec_module(measure)
real_popen=subprocess.Popen
owned=[]
def observed_popen(*args,**kwargs):
    child=real_popen(*args,**kwargs)
    if not owned:
        owned.append(child)
        (folder/'launch.tmp').write_text(json.dumps({'pid':child.pid}))
        os.replace(folder/'launch.tmp',folder/'launch.json')
        real_wait=child.wait
        first=[True]
        def interrupted_wait(*args,**kwargs):
            if first[0]:
                first[0]=False
                (folder/'waiting').write_text('ready')
                if mode!='signal':
                    deadline=time.monotonic()+5
                    while not (folder/'tree.json').exists():
                        if time.monotonic()>deadline:raise RuntimeError('fixture readiness timeout')
                        time.sleep(.01)
                    if mode=='exception':raise RuntimeError('synthetic parent exception')
                    raise KeyboardInterrupt()
            return real_wait(*args,**kwargs)
        child.wait=interrupted_wait
    return child
measure.subprocess.Popen=observed_popen
sleeper="import subprocess,sys,os,json,time; from pathlib import Path; descendant=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); Path(sys.argv[1]+'.tmp').write_text(json.dumps({'child':os.getpid(),'descendant':descendant.pid})); os.replace(sys.argv[1]+'.tmp',sys.argv[1]); time.sleep(60)"
try:
    measure.run_bounded([sys.executable,'-c',sleeper,str(folder/'tree.json')],folder,os.environ.copy(),30)
except BaseException as error:
    (folder/'result.json').write_text(json.dumps({'exception':type(error).__name__,'returncode':owned[0].returncode}))
    raise
'''


def running(pid):
    if os.name == 'nt':
        from ctypes import wintypes
        kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        kernel.OpenProcess.restype = wintypes.HANDLE
        kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
        kernel.WaitForSingleObject.restype = wintypes.DWORD
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        handle = kernel.OpenProcess(0x100000, False, pid)  # SYNCHRONIZE, fixture PIDs only.
        if not handle:
            if ctypes.get_last_error() in (87, 1168):
                return False
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            state = kernel.WaitForSingleObject(handle, 0)
            if state not in (0, 258):
                raise OSError('fixture process state unavailable')
            return state == 258
        finally:
            kernel.CloseHandle(handle)
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    # A terminated adopted descendant may briefly await the OS reaper.
    status = subprocess.run(['ps', '-o', 'stat=', '-p', str(pid)], capture_output=True,
                            text=True, timeout=2, check=False).stdout.strip()
    return bool(status) and not status.startswith('Z')


def cleanup_fixture(driver, folder):
    """Always clean only groups/PIDs created and journaled by this fixture."""
    try:
        primary = None
        pids = []
        if (folder/'launch.json').exists():
            primary = json.loads((folder/'launch.json').read_text())['pid']
            pids.append(primary)
        if (folder/'tree.json').exists():
            pids.extend(json.loads((folder/'tree.json').read_text()).values())
        for pid in set(pids):
            if type(pid) is not int or pid <= 1:
                raise AssertionError('invalid disposable fixture PID')
        if os.name == 'nt':
            for pid in set(pids):
                if running(pid):
                    try:
                        subprocess.run(['taskkill', '/PID', str(pid), '/T', '/F'],
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                       timeout=5, check=False)
                    except (OSError, subprocess.TimeoutExpired):
                        pass
                    if running(pid):
                        # Fallback only for these exact disposable fixture PIDs.
                        try:
                            os.kill(pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
        elif primary is not None:
            try:
                os.killpg(primary, signal.SIGKILL)
            except ProcessLookupError:
                pass
    finally:
        if driver.poll() is None:
            driver.kill()
        driver.wait(timeout=5)



class ProcessOwnershipTests(unittest.TestCase):
    def exercise(self, mode, expected):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            script = folder/'driver.py'; script.write_text(DRIVER)
            options = {'start_new_session': True} if os.name != 'nt' else {
                'creationflags': subprocess.CREATE_NEW_PROCESS_GROUP}
            with tempfile.TemporaryFile() as log:
                driver = subprocess.Popen([sys.executable, str(script), str(MODULE.resolve()),
                                           str(folder), mode], stdout=log, stderr=log, **options)
                try:
                    deadline = time.monotonic()+7
                    while not ((folder/'tree.json').exists() and (folder/'waiting').exists()):
                        if driver.poll() is not None or time.monotonic()>deadline:
                            self.fail('disposable process fixture failed to become ready')
                        time.sleep(.01)
                    tree = json.loads((folder/'tree.json').read_text())
                    if mode == 'signal':
                        driver.send_signal(signal.SIGINT)
                    driver.wait(timeout=12)
                    self.assertNotEqual(driver.returncode, 0)
                    result = json.loads((folder/'result.json').read_text())
                    self.assertEqual(result['exception'], expected)
                    self.assertIsNotNone(result['returncode'], 'direct child was not reaped')
                    self.assertFalse(running(tree['child']), 'disposable child survived cancellation')
                    self.assertFalse(running(tree['descendant']), 'disposable descendant survived cancellation')
                finally:
                    cleanup_fixture(driver, folder)

    @unittest.skipIf(os.name == 'nt', 'POSIX SIGINT; Windows tests inject the Python cancellation')
    def test_sigint_parent_does_not_orphan_child_or_descendant(self):
        self.exercise('signal', 'KeyboardInterrupt')

    def test_keyboard_interrupt_reaps_real_child_and_terminates_descendant(self):
        self.exercise('injected', 'KeyboardInterrupt')

    def test_parent_exception_reaps_real_child_and_terminates_descendant(self):
        self.exercise('exception', 'RuntimeError')


if __name__ == '__main__':
    unittest.main()
