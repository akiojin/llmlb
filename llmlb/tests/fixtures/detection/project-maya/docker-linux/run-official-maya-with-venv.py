import datetime, hashlib, json, pathlib, platform, shutil, subprocess, tarfile
root=pathlib.Path('/tmp');source=root/'project-maya-1.1.8'
archive=pathlib.Path('/input/project-maya-v1.1.8.tar.gz')
with tarfile.open(archive) as t:
    for x in t.getmembers():
        if not (x.name=='project-maya-1.1.8' or x.name.startswith('project-maya-1.1.8/')) or x.issym() or x.islnk() or '..' in pathlib.Path(x.name).parts:
            raise RuntimeError(x.name)
    t.extractall(root)
result={'captured_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'platform':platform.platform(),'system':platform.system(),'machine':platform.machine(),'python':platform.python_version(),'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'source_version':(source/'VERSION').read_text().strip(),'gpu_tools':{x:shutil.which(x) for x in ['nvidia-smi','nvcc','hipcc','rocminfo']},'gpu_device_nodes':[str(x) for x in pathlib.Path('/dev').iterdir() if x.name.startswith(('nvidia','dri','kfd'))],'attempts':[]}
commands=[['apt-get','update'],['apt-get','install','--no-install-recommends','-y','python3.11-venv'],['python3','-m','venv',str(source/'.venv')],['sh',str(source/'maya.sh'),'--plain','--check'],['sh',str(source/'maya.sh'),'--plain','--yes','--no-vision','--host','127.0.0.1','--port','19291'],['sh',str(source/'maya.sh'),'--plain','--yes','--no-vision','--host','127.0.0.1','--port','19292','--api-key','sk_llmlb908_maya_test_only'],['sh',str(source/'maya.sh'),'--plain','--check','--backend','hip']]
for cmd in commands:
    started=datetime.datetime.now(datetime.timezone.utc).isoformat()
    r=subprocess.run(cmd,capture_output=True,text=True,timeout=45)
    result['attempts'].append({'command':cmd,'returncode':r.returncode,'stdout':r.stdout,'stderr':r.stderr,'captured_at':started})
with tarfile.open(archive) as t:
    result['source_files_match_archive']=all((root/x.name).read_bytes()==t.extractfile(x).read() for x in t.getmembers() if x.isfile())
print(json.dumps(result,indent=2))
