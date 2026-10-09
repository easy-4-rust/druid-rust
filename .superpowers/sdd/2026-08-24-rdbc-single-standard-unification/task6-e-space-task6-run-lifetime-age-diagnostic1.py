import pathlib,json,subprocess,os,hashlib,datetime
b=pathlib.Path(__file__).parent
repo=pathlib.Path("E:/workspaces/workspace-github-easy-4-rust/druid-rust")
a=json.loads((b/"task6-lifetime-current-artifact3.summary.json").read_text());assert a["exit_code"]==0,a
arts={}
for line in (b/"task6-lifetime-current-artifact3.jsonl").read_text(encoding="utf8").splitlines():
 d=json.loads(line)
 if d.get("reason")=="compiler-artifact" and d["target"]["name"] in ("druid","druid_rdbc","tokio","async_trait"):
  name=d["target"]["name"];files=[p for p in d["filenames"] if p.endswith(".dll" if name=="async_trait" else ".rlib")];assert len(files)==1,(name,files);arts[name]=files[0]
assert len(arts)==4,arts
src=b/"task6-lifetime-age-diagnostic1.rs";exe=b/"task6-lifetime-age-diagnostic1.exe";cmd=[a["command"][0].replace("cargo.exe","rustc.exe"),"--test","--edition=2021","-Dwarnings",str(src),"-L","dependency="+a["target"]+"/debug/deps","-o",str(exe)]
for n,p in arts.items():cmd.extend(["--extern",n+"="+p])
with (b/"task6-lifetime-age-diagnostic1.compile.log").open("w",encoding="utf8") as f:r=subprocess.run(cmd,cwd=repo,stdout=f,stderr=subprocess.STDOUT)
summary={"compiler_command":cmd,"compile_exit":r.returncode,"artifact_receipt":a,"source_sha256":hashlib.sha256(src.read_bytes()).hexdigest(),"deps_sha256":{n:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest() for n,p in arts.items()},"runs":[]}
if r.returncode==0:
 for name,args,inject in [("control",["--nocapture","--test-threads=1"],False),("injected",["lifetime_original_first_idle_assertion_with_observed_age","--exact","--nocapture"],True)]:
  env=os.environ.copy()
  if inject:env["TASK6_LIFETIME_INJECT_BEFORE_CLOSE"]="1"
  else:env.pop("TASK6_LIFETIME_INJECT_BEFORE_CLOSE",None)
  run=[str(exe)]+args
  with (b/("task6-lifetime-age-diagnostic1."+name+".log")).open("w",encoding="utf8") as f:rr=subprocess.run(run,env=env,cwd=repo,stdout=f,stderr=subprocess.STDOUT)
  summary["runs"].append({"name":name,"command":run,"injected_delay_ms":30 if inject else 0,"exit_code":rr.returncode})
frozen=json.loads((b/"standard-constant-exception-inputs.json").read_text(encoding="utf8"));summary["changed_inputs"]={k:[p for p,h in frozen[k].items() if not (repo/p).exists() or hashlib.sha256((repo/p).read_bytes()).hexdigest()!=h] for k in ("inputs","control_inputs")};summary["finished_utc"]=datetime.datetime.now(datetime.timezone.utc).isoformat()
(b/"task6-lifetime-age-diagnostic1.summary.json").write_text(json.dumps(summary,indent=2),encoding="utf8");print(json.dumps(summary,indent=2))
