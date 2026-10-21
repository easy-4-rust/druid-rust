//! 用户于 2026-10-21 批准的十四项 Druid 差异；不是 JDBC 合规豁免。
const APPROVALS: &str = include_str!("../fixtures/approved_java_druid_differences.tsv");

/// 仅允许同范围、同身份、两端全部字段均与批准记录一致的差异。
pub fn approved(scope: &str, id: &str, java: &str, rust: &str) -> bool {
    APPROVALS.lines().skip(1).any(|line| {
        let fields: Vec<_> = line.split('\t').collect();
        fields.len() == 4
            && fields[0] == scope
            && fields[1] == id
            && fields[2].replace("\\t", "\t") == java
            && fields[3].replace("\\t", "\t") == rust
    })
}

#[test]
fn exact_approval_rejects_identity_scope_and_every_field_drift() {
    let mut identities = std::collections::BTreeSet::new();
    for line in APPROVALS.lines().skip(1) {
        let f: Vec<_> = line.split('\t').collect();
        assert_eq!(f.len(), 4);
        assert!(identities.insert((f[0], f[1])));
        let java = f[2].replace("\\t", "\t");
        let rust = f[3].replace("\\t", "\t");
        assert_ne!(java, rust);
        assert!(approved(f[0], f[1], &java, &rust));
        assert!(!approved("unknown", f[1], &java, &rust));
        assert!(!approved(f[0], "unknown", &java, &rust));
        for (value, is_java) in [(&java, true), (&rust, false)] {
            let fields: Vec<_> = value.split('\t').collect();
            for index in 0..fields.len() {
                let mut changed: Vec<String> = fields.iter().map(|v| (*v).to_owned()).collect();
                changed[index].push_str("_unapproved_drift");
                let changed = changed.join("\t");
                assert!(!if is_java {
                    approved(f[0], f[1], &changed, &rust)
                } else {
                    approved(f[0], f[1], &java, &changed)
                });
            }
        }
    }
    assert_eq!(identities.len(), 14);
}
