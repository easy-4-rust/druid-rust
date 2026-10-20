import com.alibaba.druid.pool.vendor.MySqlExceptionSorter;
import java.sql.SQLException;

/** 使用冻结 Druid 类验证驱动诊断文本，不改动 Java 基线源码。 */
public final class MySqlStreamingOracle {
    public static void main(String[] args) {
        MySqlExceptionSorter sorter = new MySqlExceptionSorter();
        String suffix = "is still active. No statements may be issued when any streaming result sets are "
                + "open and in use on a given connection. Ensure that you have called .close() on any "
                + "active streaming result sets before attempting more queries.";
        check(sorter, "streaming_jdbc", "Streaming result set com.mysql.jdbc.RowDataDynamic " + suffix, true);
        check(sorter, "streaming_rdbc", "Streaming result set com.mysql.rdbc.RowDataDynamic " + suffix, false);
    }

    private static void check(MySqlExceptionSorter sorter, String name, String message, boolean expected) {
        boolean actual = sorter.isExceptionFatal(new SQLException(message, null, 0));
        System.out.println(name + "\t" + actual);
        if (actual != expected) {
            throw new AssertionError(name + ": expected " + expected + ", actual " + actual);
        }
    }
}
