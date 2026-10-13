import java.sql.JDBCType;

/** 输出 Java 8 JDBCType 的真实名称、供应方和类型编号，供 Rust 差分测试逐项核对。 */
public final class Jdbc42TypesOracle {
    private Jdbc42TypesOracle() {
    }

    public static void main(String[] args) {
        for (JDBCType type : JDBCType.values()) {
            System.out.println(type.getName() + "\t" + type.getVendor() + "\t"
                    + type.getVendorTypeNumber());
        }
    }
}
