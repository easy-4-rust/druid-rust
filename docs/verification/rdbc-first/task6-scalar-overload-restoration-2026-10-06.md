# Task6 完整标量重载恢复（进行中）

## 当前全仓终态

此前稀疏恢复全仓会话93448实际终态exit101；242结果块，3709 PASS、2 FAIL、0 ignored，含2doctests。唯一失败目标task6_callable_canonical_matrix_test，两个原案例仍prepared实际5/预期6。日志task6-sparse-capability-workspace.log。该结果取代此前LIVE，但早于本轮标量描述符修改，不代表当前新源码回归。

## 完整标量控制

固定BASE的19原标量下标/列名重载、原参数、独立返回值、boolean_by_label精确DriverError、实际creatingStatement异常计数1全部接入diagnostic_tests。原生探针通过唯一read_object/read_object_by_label请求端口记录真实重载，通用value/find_column保持标准拒绝，不能以通用值表冒充19个入口。

会话57741实际exit101：现ResultSetRead缺9类标量variants，共18处E0599；这属于缺失接口的编译RED，不是行为RED。日志task6-scalar-overloads-compile-red.log。

在同一描述符增加String/Boolean/Long/Int/Short/Byte/Double/Float/Bytes；默认实现维持原value/string_value错误优先级，未新增第二driver树。现有日期资源探针对其他描述符明确拒绝，原24重载控制不变。实际行为RED会话8904正在运行，日志task6-scalar-overloads-runtime-red.log；未称通过。

账本、白名单及固定BASE分母尚未调整。公共enum增加variants会影响下游exhaustive match，API/SemVer/外部消费者与全feature/MSRV门禁继续开放。Task6 FIX_REQUIRED，完整18顶层任务继续；本轮未关闭资源、Filter、Callable、发布验收。

## 运行时 RED 与修正

同一8904终态exit101/0PASS1FAIL/90filtered。完整原案例在第一项boolean_by_label("failing")精确DriverError断言失败；因此后面的实际异常1、19重载和关闭观察尚未执行，不能提前称通过。日志task6-scalar-overloads-runtime-red.log。

ResultSetExt九类下标及九类原列名便利方法改走对应标准read描述符，不在便利层先把列名转换为下标。转换只检查一次返回值，保持原布尔所有转换、SQL NULL默认值、long严格Int、int/short/byte范围错误文本、double严格Float及float narrowing和bytes/String可空语义；不递归读取第二次。Druid同一typed Filter路径覆盖这些标量请求，成功String仍记录UTF-16长度，错误依旧由原策略分类并向实际创建Statement记录一次。

当前25162实际native-dispatch验证LIVE；测试、绑定和相关回归还未通过，公共API与wholeTask6仍开放。

同一25162 native-dispatch终态exit101/0PASS1FAIL/90filtered：原精确boolean_by_label错误、实际Statement异常1和调用记录已通过，随后普通object_by_label("object")返回UnsupportedOperation(result_set_find_column)。这证明普通对象列名也被继承默认入口提前合并。Druid现在复用既有validate_object_label列治理校验并转发raw.object_by_label，不计字符串长度，错误向实际创建Statement记一次；当前18767运行中。

独立纯度检查明确拒绝新read_object中的未限定matches!宏，没有新增检查豁免。待同一18767终态后改为std::matches!并重新验证。

## 完整原案例 GREEN

同一18767实际终态exit0/1PASS0FAIL/90filtered，完整boolean_by_label原精确错误、实际Statement.exception_count1、19原独立重载参数/值及严格调用顺序、成功后异常计数仍1和收尾全部通过。日志task6-scalar-overloads-object-label.log。先前两个runtimeRED不覆盖后续观察，现此完整GREEN才证明全案例。

将标准核心新增宏限定std::matches!后，原final纯度44检查实际通过，git diff --check通过，没有修改验证规则。当前RDBC整包回归38876 LIVE，日志task6-scalar-overloads-rdbc-regression.log；相关Druid回归和exact绑定尚未进行，1/1不能代替整包或全仓验收。

## 标准包相关回归

同一38876实际终态exit0/16结果块/471PASS/0FAIL/0ignored，含1doctest。日志task6-scalar-overloads-rdbc-regression.log。这是新增9标量描述符和便利getter后的新鲜标准包结果；原boolean全部转换/错误优先级、强getter默认合同、metadata/Wrapper等整包控制执行通过。

当前Druid相关回归31491 LIVE，日志task6-scalar-overloads-druid-regression.log，范围完整lib与7集成目标；尚无全仓/全部feature/MSRV/公共API验收，准确映射仍待Druid终态。

## Druid 回归、准确映射与当前门禁

同一31491终态exit0/8结果块/150PASS/0FAIL/0ignored：lib91、FilterStat21、metadata3、buffered6、FilterCoverage16、externalconsumer3、FilterMatrix1、typedobject9。目标1/1不重复加总。日志task6-scalar-overloads-druid-regression.log。

仅增加一个完整lib函数到原精确白名单，账本仅变动对应原案例最后4字段；1283行全部前7原BASE字段不变。该原案例改为MIGRATED_EXACT，移除旧generic矩阵仅一个同名ID，更新原反向索引和79行派生文档。现1095exact/188parameterized；自测15/15、authority79/pending25/Physical73固定SHA、方法生成14文件check、rustfmt、diff检查通过。上述矩阵ID移除后的全仓实际执行由新会话验证，不假定已通过。

同一77497完整C2检查exit1推进到rdbc_object_and_opaque_object_preserve_type_identity_and_display_contract。独立全1283源观察审计51886 exit0，1232PASS/51FAIL，仍不是语义等价证明。source-floor报告与JSON提供每项原因。

同一15535新完整workspace已启动，task6-scalar-overloads-workspace.log；未取得终态，不修改其编译输入、不重启。此前稀疏3709/2不能作为新源码结果。public enum新增variants仍有下游exhaustive match/API/SemVer/feature/MSRV与最终审查义务，不能凭471/150相关回归关闭。完整18任务/Task6 FIX_REQUIRED保留，无分支切换、暂存、提交、推送、发布、安装或升级。


## 标量全仓回归终态与后续正式案例

会话15535终态exit101，242结果块，3710PASS/2FAIL/0ignored，含2doctests。唯一失败目标task6_callable_canonical_matrix_test，两项原合同均在626行实际prepared=5、预期6；Admin整个真实gRPC组46/46PASS8.25s。日志task6-scalar-overloads-workspace.log。该结果取代LIVE，不能宣称全仓通过，也不能修改预期6来消除差异。

终态后追加两个完整对象身份及警告/元数据cfg(test)案例；生产源码没有进一步修改。新正式lib/缓冲矩阵/FilterStat回归会话1309运行中，日志task6-object-warning-formal-regression.log。账本仍未绑定两项，固定分母和51缺口统计不变。
