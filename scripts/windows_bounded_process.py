"""Windows 专用有界子进程：启动前归属 Job，限制输出并清理全部后代。"""
from __future__ import annotations
import ctypes
from ctypes import wintypes
import os
from pathlib import Path
import queue
import subprocess
import threading
import time

class WindowsBoundedProcessError(RuntimeError):
    """有界执行失败，附本次私有Job中持句柄确认的后代终止记录。"""
    def __init__(self, message: str):
        super().__init__(message)
        self.owned_descendants: tuple[dict[str, object], ...] = ()
        self.ownership_capture_error: str | None = None

def run_windows_bounded_process(command: list[str], *, cwd: Path,
                                timeout_seconds: float, output_limit: int,
                                env: dict[str, str] | None = None) -> tuple[int, str, str]:
    """返回退出码/stdout/stderr；超时、输出超限或归属失败均拒绝执行。"""
    if os.name != 'nt' or timeout_seconds <= 0 or output_limit <= 0:
        raise ValueError('Windows bounded process requires Windows and positive limits')
    import _winapi
    import msvcrt
    class BasicLimit(ctypes.Structure):
        _fields_=[('process_time',ctypes.c_int64),('job_time',ctypes.c_int64),
                  ('flags',wintypes.DWORD),('min_working_set',ctypes.c_size_t),
                  ('max_working_set',ctypes.c_size_t),('active_limit',wintypes.DWORD),
                  ('affinity',ctypes.c_size_t),('priority',wintypes.DWORD),
                  ('scheduling',wintypes.DWORD)]
    class IoCounters(ctypes.Structure):
        _fields_=[(name,ctypes.c_uint64) for name in ('read_ops','write_ops','other_ops','read_bytes','write_bytes','other_bytes')]
    class ExtendedLimit(ctypes.Structure):
        _fields_=[('basic',BasicLimit),('io',IoCounters),('process_memory',ctypes.c_size_t),
                  ('job_memory',ctypes.c_size_t),('peak_process',ctypes.c_size_t),('peak_job',ctypes.c_size_t)]
    kernel=ctypes.WinDLL('kernel32',use_last_error=True)
    kernel.CreateJobObjectW.argtypes=[ctypes.c_void_p,wintypes.LPCWSTR];kernel.CreateJobObjectW.restype=wintypes.HANDLE
    kernel.SetInformationJobObject.argtypes=[wintypes.HANDLE,ctypes.c_int,ctypes.c_void_p,wintypes.DWORD];kernel.SetInformationJobObject.restype=wintypes.BOOL
    kernel.AssignProcessToJobObject.argtypes=[wintypes.HANDLE,wintypes.HANDLE];kernel.AssignProcessToJobObject.restype=wintypes.BOOL
    kernel.TerminateJobObject.argtypes=[wintypes.HANDLE,wintypes.UINT];kernel.TerminateJobObject.restype=wintypes.BOOL
    kernel.ResumeThread.argtypes=[wintypes.HANDLE];kernel.ResumeThread.restype=wintypes.DWORD
    kernel.CloseHandle.argtypes=[wintypes.HANDLE];kernel.CloseHandle.restype=wintypes.BOOL
    kernel.QueryInformationJobObject.argtypes=[wintypes.HANDLE,ctypes.c_int,ctypes.c_void_p,wintypes.DWORD,ctypes.c_void_p];kernel.QueryInformationJobObject.restype=wintypes.BOOL
    kernel.OpenProcess.argtypes=[wintypes.DWORD,wintypes.BOOL,wintypes.DWORD];kernel.OpenProcess.restype=wintypes.HANDLE
    kernel.IsProcessInJob.argtypes=[wintypes.HANDLE,wintypes.HANDLE,ctypes.POINTER(wintypes.BOOL)];kernel.IsProcessInJob.restype=wintypes.BOOL
    class JobProcessIds(ctypes.Structure):
        _fields_=[('assigned_count',wintypes.DWORD),('listed_count',wintypes.DWORD),('ids',ctypes.c_size_t*4096)]
    job=kernel.CreateJobObjectW(None,None)
    if not job:raise ctypes.WinError(ctypes.get_last_error())
    handles=[];streams=[];threads=[];process=None;initial_thread=None;assigned=False;stop=threading.Event()
    events=queue.Queue(maxsize=16);buffers={'stdout':bytearray(),'stderr':bytearray()}
    pending_error=None;owned_handles=[]
    def send(event):
        while not stop.is_set():
            try:events.put(event,timeout=.05);return
            except queue.Full:pass
    def consume(name,stream):
        try:
            while not stop.is_set():
                chunk=stream.read(min(65536,output_limit+1))
                if not chunk:break
                send((name,chunk,None))
        except OSError as error:send((name,None,error))
        finally:send((name,None,None))
    try:
        limits=ExtendedLimit();limits.basic.flags=0x2000 # KILL_ON_JOB_CLOSE
        if not kernel.SetInformationJobObject(job,9,ctypes.byref(limits),ctypes.sizeof(limits)):
            raise ctypes.WinError(ctypes.get_last_error())
        out_read,out_write=_winapi.CreatePipe(None,0);handles.extend((out_read,out_write))
        err_read,err_write=_winapi.CreatePipe(None,0);handles.extend((err_read,err_write))
        for handle in (out_write,err_write):os.set_handle_inheritable(handle,True)
        stdin_fd=os.open(os.devnull,os.O_RDONLY)
        try:
            current=_winapi.GetCurrentProcess()
            stdin_handle=_winapi.DuplicateHandle(current,msvcrt.get_osfhandle(stdin_fd),current,0,True,_winapi.DUPLICATE_SAME_ACCESS)
        finally:os.close(stdin_fd)
        handles.append(stdin_handle)
        startup=subprocess.STARTUPINFO();startup.dwFlags=subprocess.STARTF_USESTDHANDLES|subprocess.STARTF_USESHOWWINDOW;startup.wShowWindow=0
        startup.hStdInput=stdin_handle;startup.hStdOutput=out_write;startup.hStdError=err_write
        startup.lpAttributeList={'handle_list':[stdin_handle,out_write,err_write]}
        process,initial_thread,pid,tid=_winapi.CreateProcess(None,subprocess.list2cmdline(command),None,None,True,
            0x4|subprocess.CREATE_NO_WINDOW|0x80000,env,str(cwd),startup)
        # 初始线程尚未执行用户代码；加入私有job后才允许创建后代，杜绝归属窗口。
        if not kernel.AssignProcessToJobObject(job,process):raise ctypes.WinError(ctypes.get_last_error())
        assigned=True
        for handle in (stdin_handle,out_write,err_write):_winapi.CloseHandle(handle);handles.remove(handle)
        for name,handle in (('stdout',out_read),('stderr',err_read)):
            descriptor=msvcrt.open_osfhandle(handle,os.O_RDONLY|os.O_BINARY);handles.remove(handle)
            stream=os.fdopen(descriptor,'rb',buffering=0);streams.append(stream)
            thread=threading.Thread(target=consume,args=(name,stream),daemon=True);threads.append(thread);thread.start()
        if kernel.ResumeThread(initial_thread)==0xffffffff:raise ctypes.WinError(ctypes.get_last_error())
        # 与原POSIX执行器一致：子进程已启动且输出管道已就绪后开始运行deadline。
        deadline=time.monotonic()+timeout_seconds
        _winapi.CloseHandle(initial_thread);initial_thread=None
        ended=set();terminated_descendants=False
        while len(ended)<2 or _winapi.WaitForSingleObject(process,0)!=_winapi.WAIT_OBJECT_0:
            remaining=deadline-time.monotonic()
            if remaining<=0:raise RuntimeError(f'evidence process timed out after {timeout_seconds:.3f}s')
            if _winapi.WaitForSingleObject(process,0)==_winapi.WAIT_OBJECT_0 and not terminated_descendants:
                if not kernel.TerminateJobObject(job,1):raise ctypes.WinError(ctypes.get_last_error())
                terminated_descendants=True
            try:name,chunk,error=events.get(timeout=min(.05,remaining))
            except queue.Empty:continue
            if error is not None:raise error
            if chunk is None:ended.add(name);continue
            if sum(len(value) for value in buffers.values())+len(chunk)>output_limit:
                raise RuntimeError(f'evidence process exceeded output limit {output_limit}')
            buffers[name].extend(chunk)
        returncode=_winapi.GetExitCodeProcess(process)
        return returncode,bytes(buffers['stdout']).decode('utf-8',errors='replace'),bytes(buffers['stderr']).decode('utf-8',errors='replace')
    except RuntimeError as error:
        pending_error=WindowsBoundedProcessError(str(error))
        raise pending_error from error
    finally:
        # 只枚举这个私有Job；提前持有同步句柄并验证归属，避免PID复用假证据。
        if pending_error is not None and assigned:
            try:
                members=JobProcessIds()
                if not kernel.QueryInformationJobObject(job,3,ctypes.byref(members),ctypes.sizeof(members),None):
                    raise ctypes.WinError(ctypes.get_last_error())
                if members.listed_count>4096:
                    raise RuntimeError('owned job membership diagnostic exceeds bounded capacity')
                for index in range(members.listed_count):
                    member_pid=int(members.ids[index])
                    if member_pid==pid:continue
                    handle=kernel.OpenProcess(0x101000,False,member_pid)
                    if not handle:
                        # 已退出/不可查询时保留capture错误，不能伪造已终止证据。
                        raise ctypes.WinError(ctypes.get_last_error())
                    inside=wintypes.BOOL()
                    if not kernel.IsProcessInJob(handle,job,ctypes.byref(inside)) or not inside.value:
                        kernel.CloseHandle(handle)
                        raise RuntimeError('owned job descendant identity is no longer verified')
                    owned_handles.append((member_pid,handle))
            except (OSError,RuntimeError) as error:
                pending_error.ownership_capture_error=str(error)
        # 仅私有job及本次创建的进程句柄；不按名称或旧PID操作外部进程。
        if process is not None:
            if assigned:
                kernel.TerminateJobObject(job,1)
            elif _winapi.WaitForSingleObject(process,0)!=_winapi.WAIT_OBJECT_0:
                _winapi.TerminateProcess(process,1)
            _winapi.WaitForSingleObject(process,5000)
        stop.set();kernel.CloseHandle(job)
        cleanup_deadline=time.monotonic()+5.0
        descendant_records=[]
        for member_pid,handle in owned_handles:
            remaining_ms=max(0,int((cleanup_deadline-time.monotonic())*1000))
            state=_winapi.WaitForSingleObject(handle,remaining_ms)
            descendant_records.append(dict(process_id=member_pid,private_job_membership_verified=True,terminated=state==_winapi.WAIT_OBJECT_0))
            kernel.CloseHandle(handle)
        if pending_error is not None:
            pending_error.owned_descendants=tuple(descendant_records)
        for thread in threads:thread.join(timeout=2)
        for stream in streams:stream.close()
        for handle in handles:_winapi.CloseHandle(handle)
        if initial_thread is not None:_winapi.CloseHandle(initial_thread)
        if process is not None:_winapi.CloseHandle(process)
