use serde::{Deserialize, Serialize};
use serde_json::Value;

/// SQL 防火墙统计响应。
///
/// 对应 Java: `com.alibaba.druid.admin.model.dto.WallResult`。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallResult {
    #[serde(rename = "ResultCode")]
    pub result_code: i32,
    #[serde(rename = "Content", default)]
    pub content: WallContent,
}

/// `WallResult.ContentBean` 的 Rust 表达。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WallContent {
    pub check_count: i64,
    pub hard_check_count: i64,
    pub violation_count: i64,
    pub violation_effect_row_count: i64,
    pub black_list_hit_count: i64,
    pub black_list_size: i64,
    pub white_list_hit_count: i64,
    pub white_list_size: i64,
    pub syntax_error_count: i64,
    #[serde(default)]
    pub tables: Option<Vec<WallTable>>,
    #[serde(default)]
    pub functions: Option<Vec<WallFunction>>,
    #[serde(default)]
    pub black_list: Option<Vec<Value>>,
    #[serde(default)]
    pub white_list: Option<Vec<WallWhiteList>>,
}

/// `WallResult.ContentBean.TablesBean` 的 Rust 表达。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WallTable {
    pub name: Option<String>,
    pub select_count: i64,
    pub fetch_row_count: i64,
    #[serde(default)]
    pub fetch_row_count_histogram: Option<Vec<i64>>,
}

/// `WallResult.ContentBean.FunctionsBean` 的 Rust 表达。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WallFunction {
    pub name: Option<String>,
    pub invoke_count: i64,
}

/// `WallResult.ContentBean.WhiteListBean` 的 Rust 表达。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WallWhiteList {
    pub sql: Option<String>,
    pub sample: Option<String>,
    pub execute_count: i64,
    pub fetch_row_count: i64,
}

impl WallResult {
    /// 将另一个节点的结果累加到当前对象。
    ///
    /// 对应 Java: `WallResult#sum`。计数字段求和，四个列表保持节点遍历
    /// 顺序追加；远端 `null` 列表按空列表处理。
    pub fn sum(&mut self, wall_result: &Self) {
        self.content.check_count += wall_result.content.check_count;
        self.content.hard_check_count += wall_result.content.hard_check_count;
        self.content.violation_count += wall_result.content.violation_count;
        self.content.violation_effect_row_count += wall_result.content.violation_effect_row_count;
        self.content.black_list_hit_count += wall_result.content.black_list_hit_count;
        self.content.black_list_size += wall_result.content.black_list_size;
        self.content.white_list_hit_count += wall_result.content.white_list_hit_count;
        self.content.white_list_size += wall_result.content.white_list_size;
        self.content.syntax_error_count += wall_result.content.syntax_error_count;
        append(
            &mut self.content.tables,
            wall_result.content.tables.as_ref(),
        );
        append(
            &mut self.content.functions,
            wall_result.content.functions.as_ref(),
        );
        append(
            &mut self.content.black_list,
            wall_result.content.black_list.as_ref(),
        );
        append(
            &mut self.content.white_list,
            wall_result.content.white_list.as_ref(),
        );
    }
}

fn append<T: Clone>(target: &mut Option<Vec<T>>, source: Option<&Vec<T>>) {
    target
        .get_or_insert_with(Vec::new)
        .extend(source.into_iter().flatten().cloned());
}
