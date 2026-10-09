"""Build real Array proxy in an isolated full-source candidate after 28/5 RED."""
from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import os

base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
original=base/'task7-full-druid-source-candidate1'
candidate=base/'task7-array-source-candidate1'
candidate.mkdir(exist_ok=False)
for name in ('src','resources'): shutil.copytree(original/name,candidate/name)
core=candidate/'src/core'
state=(original/'src/core/druid_blob_state.rs').read_text(encoding='utf-8')
state=state.replace('DruidBlobState','DruidArrayState').replace('resource::Blob','resource::Array').replace('dyn Blob','dyn Array')
state=state.replace('二进制大对象','SQL数组').replace('com.alibaba.druid.proxy.jdbc.BlobProxyImpl','java.sql.Array（池化资源状态拆分）')
state=state.replace('    close_gate: tokio::sync::Mutex<()>,','    close_gate: tokio::sync::Mutex<()>,\n    children: Mutex<Vec<Arc<super::druid_pooled_result_set::StandardResultSetResource>>>,')
state=state.replace('            close_gate: tokio::sync::Mutex::new(()),','            close_gate: tokio::sync::Mutex::new(()),\n            children: Mutex::new(Vec::new()),')
needle='    /// 返回已登记令牌；未完成登记时返回 None。'
state=state.replace(needle,'''    /// 同步接管新游标状态；参数为驱动游标的监管资源，无返回值。
    /// 对应 Java: Array#getResultSet 的独立资源交接。
    pub(crate) fn track_cursor(&self, cursor: Arc<super::druid_pooled_result_set::StandardResultSetResource>) {
        self.children.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(cursor);
    }
'''+needle)
needle='''        let result = self
            .context
            .release_with(|| async { self.payload.lock().await.free().await })
            .await;'''
assert state.count(needle)==1
state=state.replace(needle,'''        // 先等待当前 Array 操作交接游标，随后直接关闭子 actor，不能排队等待本协调器。
        let mut payload = self.payload.lock().await;
        let children = self.children.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        for child in children { child.close_core().await?; }
        let result = self.context.release_with(|| async { payload.free().await }).await;''')
(core/'druid_array_state.rs').write_text(state,encoding='utf-8',newline='\n')

proxy='''use super::druid_array_state::DruidArrayState;
use super::resource_context::ResourceContext;
use super::resource_runtime_capabilities::ResourceRuntimeCapabilities;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registry::StandardResourceRegistry;
use super::standard_logical_policy::StandardLogicalPolicy;
use super::filter_chain::FilterChain;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{Array,ResourceOperations};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::types::{RdbcObject,RdbcTypeMap};
use std::sync::{Arc,atomic::{AtomicBool,AtomicU64}};

/// 当前连接租约监管的 SQL 数组代理，保留元素、映射及游标身份。
/// 对应 Java: java.sql.Array；承担 Druid pooled 资源代理职责。
pub(crate) struct DruidArray {
    state: Arc<DruidArrayState>,
    registry: Arc<StandardResourceRegistry>,
    capabilities: ResourceOperations,
    connection_id: u64,
    result_set_id_seed: Arc<AtomicU64>,
    lease_active: Arc<AtomicBool>,
    filter_chain: Option<Arc<FilterChain>>,
    stats_collector: Option<Arc<crate::stats::StatsCollector>>,
    policy: Arc<StandardLogicalPolicy>,
}
impl std::fmt::Debug for DruidArray {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        f.debug_struct("DruidArray").field("state",&self.state.context.state()).field("capabilities",&self.capabilities).finish_non_exhaustive()
    }
}
impl DruidArray {
    /// 接管数组与当前租约的游标上下文，登记后返回监管代理；失败返回原登记错误。
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn register(payload:Box<dyn Array>,context:Arc<ResourceContext>,registry:Arc<StandardResourceRegistry>,
        admission:&StandardResourceAdmission,connection_id:u64,result_set_id_seed:Arc<AtomicU64>,lease_active:Arc<AtomicBool>,
        filter_chain:Option<Arc<FilterChain>>,stats_collector:Option<Arc<crate::stats::StatsCollector>>,policy:Arc<StandardLogicalPolicy>) -> Result<Self,RdbcError> {
        let supported=payload.capabilities(); let mut capabilities=ResourceOperations::empty();
        for (operation,runtime) in [(ResourceOperations::READ,ResourceRuntimeCapabilities::READ),
            (ResourceOperations::RANGE,ResourceRuntimeCapabilities::RANGE),(ResourceOperations::TYPE_MAP,ResourceRuntimeCapabilities::TYPE_MAP),
            (ResourceOperations::RESULT_SET,ResourceRuntimeCapabilities::RESULT_SET),(ResourceOperations::FREE,ResourceRuntimeCapabilities::FREE)] {
            if supported.contains(operation) && context.capabilities().contains(runtime) { capabilities |= operation; }
        }
        let state=Arc::new(DruidArrayState::new(payload,context));
        let actor:Arc<dyn StandardResourceHandle>=state.clone();
        let registered=registry.register_batch(admission,vec![actor]).await; admission.finish(); registered?;
        Ok(Self {state,registry,capabilities,connection_id,result_set_id_seed,lease_active,filter_chain,stats_collector,policy})
    }
    fn require(&self,capability:ResourceOperations,operation:&'static str)->Result<(),RdbcError> {
        self.state.context.require(ResourceRuntimeCapabilities::empty(),operation)?;
        if self.state.registration().is_none_or(|registration| registration.cleanup_requested()) {
            return Err(RdbcError::rdbc_resource_closed("Array"));
        }
        if !self.capabilities.contains(capability) { return Err(RdbcError::feature_not_supported(operation)); } Ok(())
    }
    async fn payload(&self,capability:ResourceOperations,operation:&'static str)->Result<tokio::sync::MutexGuard<'_,Box<dyn Array>>,RdbcError> {
        self.require(capability,operation)?; let payload=self.state.payload.lock().await; self.require(capability,operation)?; Ok(payload)
    }
}
#[async_trait::async_trait]
impl Array for DruidArray {
    fn capabilities(&self)->ResourceOperations { self.capabilities }
'''
methods=[('base_type_name','','String','READ',''),('base_type','','i32','READ',''),('values','','Vec<RdbcObject>','READ',''),
 ('values_with_type_map','type_map:&RdbcTypeMap','Vec<RdbcObject>','READ | ResourceOperations::TYPE_MAP','type_map'),
 ('values_range','index:i64,count:i32','Vec<RdbcObject>','READ | ResourceOperations::RANGE','index,count'),
 ('values_range_with_type_map','index:i64,count:i32,type_map:&RdbcTypeMap','Vec<RdbcObject>','READ | ResourceOperations::RANGE | ResourceOperations::TYPE_MAP','index,count,type_map')]
for name,params,result,cap,args in methods:
    proxy+=f'''    async fn {name}(&mut self{',' if params else ''}{params})->Result<{result},RdbcError> {{
        let result=self.payload(ResourceOperations::{cap},"Array#{name}").await?.{name}({args}).await;
        self.state.context.classify_operation_result(result)
    }}
'''
for name,params,cap,args in [
 ('result_set','','RESULT_SET',''),('result_set_with_type_map','type_map:&RdbcTypeMap','RESULT_SET | ResourceOperations::TYPE_MAP','type_map'),
 ('result_set_range','index:i64,count:i32','RESULT_SET | ResourceOperations::RANGE','index,count'),
 ('result_set_range_with_type_map','index:i64,count:i32,type_map:&RdbcTypeMap','RESULT_SET | ResourceOperations::RANGE | ResourceOperations::TYPE_MAP','index,count,type_map')]:
    proxy+=f'''    async fn {name}(&mut self{',' if params else ''}{params})->Result<Box<dyn ResultSet>,RdbcError> {{
        let mut payload=self.payload(ResourceOperations::{cap},"Array#{name}").await?;
        let admission=self.registry.begin_admission(self.policy.resource_generation())?;
        let raw_result=payload.{name}({args}).await;
        let raw=self.state.context.classify_operation_result(raw_result)?;
        let state=Arc::clone(&self.state);
        // 在第一次可能挂起前保存子游标，然后释放 Array 锁；失败清理不等待自身持有的锁。
        let result=super::druid_pooled_result_set::DruidPooledResultSet::new_standard_metadata_with_handoff(
            self.connection_id,Arc::clone(&self.result_set_id_seed),Arc::clone(&self.lease_active),self.filter_chain.clone(),
            self.stats_collector.clone(),Arc::clone(&self.registry),Arc::clone(&self.policy),&admission,raw,
            move |cursor| {{ state.track_cursor(cursor); drop(payload); }}).await;
        admission.finish();
        result.map(|cursor| Box::new(cursor) as Box<dyn ResultSet>)
    }}
'''
proxy+='''    async fn free(&mut self)->Result<(),RdbcError> {
        if let Some(result)=self.state.completed_release().await { return result; }
        let registration=self.state.registration().ok_or(RdbcError::ConnectionDiscarded)?;
        self.registry.close_registered(std::slice::from_ref(registration)).await
    }
}
'''
(core/'druid_array.rs').write_text(proxy,encoding='utf-8',newline='\n')

path=core/'mod.rs'; text=path.read_text(encoding='utf-8'); assert 'mod druid_array;' not in text
path.write_text('mod druid_array;\nmod druid_array_state;\n'+text,encoding='utf-8',newline='\n')

path=core/'druid_pooled_result_set.rs'; text=path.read_text(encoding='utf-8')
needle='''        let resources = Arc::clone(&statement.resources);
        let staged = StagedResultSet::new(statement, raw);
        let handle = Arc::clone(&staged.resource) as Arc<dyn StandardResourceHandle>;'''
assert text.count(needle)==1
text=text.replace(needle,'''        Self::new_standard_auxiliary_with_handoff(statement, raw, admission, |_| {}).await
    }

    async fn new_standard_auxiliary_with_handoff<F>(
        statement: Arc<DruidPooledStatementInner>, raw: Box<dyn ResultSet>,
        admission: &StandardResourceAdmission, handoff: F,
    ) -> Result<Self, RdbcError>
    where F: FnOnce(Arc<StandardResultSetResource>) + Send {
        let resources = Arc::clone(&statement.resources);
        let staged = StagedResultSet::new(statement, raw);
        // 将已接管 raw 的 actor 同步交给资源 owner，再进入异步 registry 登记。
        handoff(Arc::clone(&staged.resource));
        let handle = Arc::clone(&staged.resource) as Arc<dyn StandardResourceHandle>;''')
start=text.index('    #[allow(clippy::too_many_arguments)]\n    pub(crate) async fn new_standard_metadata(')
end=text.index('    /// Returns the result-set identifier.',start)
old=text[start:end]
signature=old[:old.index('        let statement =')]
delegate='''        Self::new_standard_metadata_with_handoff(connection_id,result_set_id_seed,lease_active,filter_chain,
            stats_collector,resources,policy,admission,raw,|_| {}).await
    }

    /// 同步交接无创建语句的资源游标，返回受原连接租约监管的完整标准代理。
    /// 对应 Java: Array#getResultSet；handoff 在 registry 可能挂起前接管子 actor。
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new_standard_metadata_with_handoff<F>(
        connection_id:u64,result_set_id_seed:Arc<AtomicU64>,lease_active:Arc<AtomicBool>,
        filter_chain:Option<Arc<FilterChain>>,stats_collector:Option<Arc<crate::stats::StatsCollector>>,
        resources:Arc<StandardResourceRegistry>,policy:Arc<super::standard_logical_policy::StandardLogicalPolicy>,
        admission:&StandardResourceAdmission,raw:Box<dyn ResultSet>,handoff:F,
    )->Result<Self,RdbcError>
    where F:FnOnce(Arc<StandardResultSetResource>)+Send {
'''
body=old[old.index('        let statement ='):]
body=body.replace('''        let mut result = Self::new_standard(statement.inner, raw, admission).await?;''','''        let mut result = Self::new_standard_auxiliary_with_handoff(Arc::clone(&statement.inner),raw,admission,handoff).await?;
        statement.inner.set_execution_results(vec![TrackedStatementResult::ResultSet(Arc::downgrade(&result.resource))]);''')
text=text[:start]+signature+delegate+body+text[end:]
path.write_text(text,encoding='utf-8',newline='\n')

path=core/'druid_pooled_connection.rs'; text=path.read_text(encoding='utf-8')
needle='    async fn commit(&mut self) -> Result<(), RdbcError> {'
assert text.count(needle)==1
method='''    /// 创建当前租约监管的 SQL 数组，保留可空类型名和有序元素；返回资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createArrayOf。
    async fn create_array_of(&mut self,type_name:Option<&str>,elements:Option<Vec<druid_rdbc::types::RdbcObject>>)
        ->Result<Box<dyn druid_rdbc::resource::Array>,RdbcError> {
        self.standard_ref()?;
        let registry=self.holder.as_ref().ok_or(RdbcError::ConnectionDiscarded)?.standard_resource_registry();
        let policy=self.standard_policy.as_ref().ok_or(RdbcError::ConnectionDiscarded)?.clone();
        policy.ensure_open()?; let admission=registry.begin_admission(policy.resource_generation())?;
        let raw_result=self.standard_mut()?.create_array_of(type_name,elements).await;
        let raw=self.classify_result(raw_result)?;
        let id=super::resource_id::ResourceId::local();
        let owner=Arc::new(super::druid_resource_owner::DruidResourceOwner::new(id.clone(),super::resource_kind::ResourceKind::Array,policy.clone()));
        let context=Arc::new(super::resource_context::ResourceContext::new(id,super::resource_kind::ResourceKind::Array,
            super::resource_runtime_capabilities::ResourceRuntimeCapabilities::READ
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::RANGE
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::TYPE_MAP
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::RESULT_SET
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::FREE,owner));
        let resource=super::druid_array::DruidArray::register(raw,context,registry,&admission,self.id,
            Arc::clone(&self.result_set_id_seed),Arc::clone(&self.lease_active),self.filter_chain.clone(),self.stats_collector.clone(),policy).await?;
        Ok(Box::new(resource))
    }

'''
path.write_text(text.replace(needle,method+needle),encoding='utf-8',newline='\n')

tool=Path('C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin')
files=[core/name for name in ('druid_array.rs','druid_array_state.rs','druid_pooled_connection.rs','druid_pooled_result_set.rs','mod.rs')]
fmt=subprocess.run([str(tool/'rustfmt.exe'),'--edition','2021', '--config','skip_children=true',*[str(p) for p in files]],capture_output=True)
(candidate/'rustfmt.log').write_bytes(fmt.stdout+fmt.stderr)
if fmt.returncode: raise SystemExit(fmt.returncode)
baseline=json.loads((base/'task7-array-baseline-coherent1/summary.json').read_text(encoding='utf-8'))
record=next(row for row in baseline['commands'] if row['name']=='druid')
command=record['command'].copy()
for i,arg in enumerate(command):
    if arg==str(original/'src/lib.rs'): command[i]=str(candidate/'src/lib.rs')
    if arg=='metadata=task7_array_coherent_druid1': command[i]='metadata=task7_pooled_array_green1'
command[command.index('-o')+1]=str(candidate/'libdruid.rlib')
env=os.environ.copy(); env.update(record['environment']); env['CARGO_MANIFEST_DIR']=str(candidate)
with (candidate/'compile.log').open('xb') as output:
    result=subprocess.run(command,env=env,stdout=output,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
summary={'command':command,'environment':record['environment'] | {'CARGO_MANIFEST_DIR':str(candidate)},
         'exit_code':result.returncode,'production_applied':False,
         'sources':{p.relative_to(candidate).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((candidate/'src').rglob('*.rs'))}}
(candidate/'compile.summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'compile_exit':result.returncode,'candidate':str(candidate)}),flush=True)
raise SystemExit(result.returncode)
