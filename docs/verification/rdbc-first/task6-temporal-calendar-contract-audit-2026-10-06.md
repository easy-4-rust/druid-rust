# Task 6：Decimal、temporal 与 Calendar 完整原案例核查（2026-10-06）

固定 BASE 原案例 strong_typed_getters_preserve_all_decimal_temporal_and_calendar_overloads 已逐段读取，当前仍为参数化待恢复项，不修改处置、分母或完成数。此核查用于准备下一项完整迁移，不能替代执行证据。

原数据：Decimal(1234567890.45)、Date(2026-07-29)、Time(13:14:15.123456789)、Timestamp(上述Date+Time)、NULL；列名 amount/event_date/event_time/event_at/optional_amount。通过真实 SQLite pooled Connection 创建 Statement，再将具有原数据的缓冲结果集纳入真实池化资源注册路径。

保留全部观察：next成功；元数据前四列分别Decimal/Date/Time/Timestamp；Decimal下标/列名各有无scale两重载，scale=2；Date下标/列名及指定上海Calendar/显式None；Time下标/列名及显式None/上海Calendar；Timestamp下标/列名及上海Calendar/显式None；optional_amount读取None，was_null=true。不得以强getter参数身份probe或十三类getObject转换测试替代这组实际返回值和metadata观察。

canonical入口映射：ColumnIndex保留1-based下标；旧无Calendar重载转换为RdbcCalendarArgument::Unspecified，旧显式Calendar重载转换为Specified(Some(calendar))/Specified(None)，不折叠两种None语义。ResultSetExt的date/time/timestamp及按label入口委托read_object/read_object_by_label；同一BufferedRowSet负责metadata和实际值。现有全部强getter身份测试只能补充参数身份，不能证明本原案例已执行。

本轮fullworkspace session42807在F盘独立CARGO_TARGET_DIR持续运行，已通过句柄确认LIVE，未重启、取消或改写Rust。下一次先检查同一句柄终态；终态前不向Cargo输入源码追加本案例。完整元数据wrapper身份案例仍是C2第一缺口，未缩小或代替其验收。

原始身份：crates/druid/tests/druid_pooled_result_set_semantics_test.rs#strong_typed_getters_preserve_all_decimal_temporal_and_calendar_overloads
原始SHA：96e097c165ea6ab8bad36a405668f4157b12dbab6df202847f6ff71179da7087
原断言库存：assert:2;assert_eq:21
原调用库存：Date:1;Decimal:1;Some:19;Time:1;Timestamp:1;big_decimal:1;big_decimal_by_label:2;big_decimal_by_label_with_scale:1;big_decimal_with_scale:1;clone:6;column_type:4;create_statement:1;date:1;date_by_label:1;date_by_label_with_calendar:1;date_with_calendar:1;from_hms_nano_opt:1;from_str:1;from_ymd_opt:1;meta_data:1;new:4;next:1;sqlite_pooled_connection:1;time:1;time_by_label:1;time_by_label_with_calendar:1;time_with_calendar:1;timestamp:1;timestamp_by_label:1;timestamp_by_label_with_calendar:1;timestamp_with_calendar:1;to_string:5;unwrap:30;was_null:1;with_column_labels:1;wrap_result_set:1
