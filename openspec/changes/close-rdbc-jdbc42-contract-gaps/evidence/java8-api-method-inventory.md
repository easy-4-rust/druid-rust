# MC-01：冻结 Java 8 API 方法清单

## 结果

任务 8.1 已完成。`scripts/generate_java8_api_method_inventory.py` 使用 JDK 8 `javadoc` 执行 `JdbcApiInventoryDoclet`，输入固定 OpenJDK `jdk8u502-b07` 源码（commit `7373384725418fa645fb4508ec05a869bc7fc3e7`，归档 SHA-256 `5f9c0657c534319a272751431a1f5919cfdcd4e0ac4e0fb9b7e181bc27fa489b`）。提取器只处理归档内 52 个 `java.sql` 和 17 个 `javax.sql` 直接包源码，不读取 Rust 实现或现有 Rust 方法表；归档及解压源码只存在临时目录，不进入仓库。

生成的纯文本清单位于 `docs/verification/rdbc-first/java8-public-method-inventory.tsv`：69 个类型，2,001 条记录，SHA-256 `27F424D0FC4262D8862F93044BD61D35A5BAB3FD85511D58D5CCE5DEB7FFE4F5`。记录构造器、公开常量、每个重载、Java 8 default 方法、继承接收方/声明类型、源码相对路径和行号。类别计数：type 69、method 1,060、constructor 148、constant 184、inherited_method 513、inherited_constant 27。

8.2 台账接入时发现 `JDBCType` 对 `SQLType` 三个同签名重声明与继承声明存在身份碰撞。生成器现为继承成员身份键加上 `@<declared_in>`，修复后重新从固定归档生成；记录数不变，唯一身份数为 2,001。增加的 Java-only fixture 覆盖接收类型自身重声明与继承声明同时保留，修订后整个 inventory 工具测试为 5 项通过。以下新身份规则已纳入清单 SHA-256。

## 行为与独立性验证

- `python -m unittest discover -s tools/java8-api-inventory -p 'test_*.py' -v`：5 passed。测试通过 JDK 8 自定义 doclet运行 Java-only fixture，确认 `ConnectionEventListener` 未被漏掉、两个 listener 回调均出现；方法重载身份分离；显式重载及隐式公开构造器收录；父类方法、父接口 default 方法、继承常量保留原始 declaring type；继承声明与接收类型同签名重声明不碰撞；private 方法排除；输出不引用 Rust 路径。另有证据门禁四项控制测试。
- 同一 fixture 在独立临时输出位置重复生成，TSV 字节完全相同。
- `python -m py_compile scripts/generate_java8_api_method_inventory.py tools/java8-api-inventory/test_inventory_doclet.py`：退出码 0。
- `python scripts/generate_java8_api_method_inventory.py` 连续执行两次：均退出码 0；固定归档 SHA-256 校验通过，均报告 69 个源文件和 2,001 条记录，输出 SHA-256 一致。
- 运行 Javadoc/Javac：Oracle JDK 8u321-b07（`D:\Java\jdk1.8.0_321`）；Java 8u321 是执行工具版本，不替代精确固定的 8u502 源码身份。
- OpenSpec 严格验证和 `git diff --check` 在提交前执行。

此项建立独立的 Java 来源分母，不表示 Rust 适配器、逻辑代理或真实后端方法均已实现；映射/行为证据与资源矩阵由后续 8.2–8.8 验收。

## 文件身份（SHA-256）

```text
tools/java8-api-inventory/JdbcApiInventoryDoclet.java BC33CB1DEC4E70769C711891B74C5F954CA31C3CD529ADA633C283FE1469C830
tools/java8-api-inventory/test_inventory_doclet.py 7EBAA1750DF7E8819636B32B7B7B8D4B5FC08CCD1BE92BF2802117D90E24769D
tools/java8-api-inventory/README.md 070A4C7A595265F11267C7AE7B97364AB3F7663FC6506FA3916BE5360E44BE6C
tools/java8-api-inventory/fixtures/fixture/api/ConnectionEventListener.java DD52F0D149A6C29EC1FB1490AEA5226CD6315AA5184F5B48DB053C9FE7717EA1
tools/java8-api-inventory/fixtures/fixture/api/InheritedContract.java F2DA94F1AC8EABB7EFF5E996722166DA3B1EAA8F5706E0B34ACF86E6F4FFC9E9
tools/java8-api-inventory/fixtures/fixture/api/BaseOwner.java DFD1860201C93BE5311DB8280CCF8E7C69F88498C45D90F4B5B9BC68EF682B78
tools/java8-api-inventory/fixtures/fixture/api/PooledOwner.java 7F212AD88D22A7A25DA6E40EA77166EBB2AF8D9997D84641B7C3E36405B42FC6
tools/java8-api-inventory/fixtures/fixture/api/ImplicitConstructorApi.java 980A403DC45F425BBC6123BEA60172A932E98D690459629B4C4B1162AFC7BF52
scripts/generate_java8_api_method_inventory.py 5BF8FAF7E310CFA0EC4E5799CEAFEE7D23148E0D572CC9DDF282813B7FD504F2
docs/verification/rdbc-first/java8-public-method-inventory.tsv 27F424D0FC4262D8862F93044BD61D35A5BAB3FD85511D58D5CCE5DEB7FFE4F5
```
