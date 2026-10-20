import com.alibaba.druid.pool.DruidDataSource;
import com.alibaba.druid.pool.DruidPooledConnection;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.sql.Connection;
import java.sql.SQLException;
import java.sql.Statement;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;

/** 原始三类六例的 setup、断言和真实池状态；不将任何错误归一成成功。 */
public final class PoolCasesOracle {
    private static final String PACKAGE = "com.alibaba.druid.bvt.pool.";
    private PoolCasesOracle() {}
    private static void lifecycle(Object test, String method) throws Exception {
        Method entry = test.getClass().getDeclaredMethod(method);
        entry.setAccessible(true);
        entry.invoke(test);
    }
    private static Object start(String name) throws Exception {
        Object test = Class.forName(PACKAGE + name).getDeclaredConstructor().newInstance();
        lifecycle(test, "setUp");
        return test;
    }
    private static String utf16(String value) {
        if (Objects.isNull(value)) { return "-"; }
        StringBuilder result = new StringBuilder();
        for (int index = 0; index < value.length(); index++) {
            result.append(String.format("%04x", (int) value.charAt(index)));
        }
        return result.toString();
    }
    private static String observe(String[] input) throws Exception {
        Object original = start(input[1]);
        try { original.getClass().getMethod(input[2]).invoke(original); }
        finally { lifecycle(original, "tearDown"); }
        Object test = start(input[1]);
        try {
            Field field = test.getClass().getDeclaredField("dataSource");
            field.setAccessible(true);
            DruidDataSource source = (DruidDataSource) field.get(test);
            if (source.getMaxActive() != Integer.parseInt(input[8])
                    || source.getProxyFilters().isEmpty() == Boolean.parseBoolean(input[6])) {
                throw new IllegalArgumentException("original pool setup differs");
            }
            Connection connection = null;
            Statement statement = null;
            Object raw = null;
            Exception failure = null;
            int activeDuring = 0;
            int idleDuring = 0;
            try {
                connection = input[1].endsWith("getConnection")
                    ? source.getConnection("-".equals(input[3]) ? null : input[3], "-".equals(input[4]) ? null : input[4])
                    : source.getConnection();
                raw = ((DruidPooledConnection) connection).getConnection();
                if (!"-".equals(input[7])) {
                    if (!Boolean.parseBoolean(input[5])) {
                        connection.setAutoCommit(false);
                        connection.setReadOnly(false);
                    }
                    statement = connection.createStatement();
                    statement.execute(input[7]);
                }
                activeDuring = source.getActiveCount();
                idleDuring = source.getPoolingCount();
                connection.close();
            } catch (Exception error) { failure = error; }
            int activeAfter = source.getActiveCount();
            int idleAfter = source.getPoolingCount();
            String statementClosed = Objects.isNull(statement) ? "-" : Boolean.toString(statement.isClosed());
            String connectionClosed = Objects.isNull(connection) ? "-" : Boolean.toString(connection.isClosed());
            String same = "-";
            if (Objects.isNull(failure)) {
                DruidPooledConnection replacement = source.getConnection();
                same = Boolean.toString(raw == replacement.getConnection());
                replacement.close();
            }
            String diagnostics = Objects.isNull(failure) ? "RETURN\t-\t-\t-\t-"
                : "THROW\t" + failure.getClass().getName() + "\t"
                    + (failure instanceof SQLException ? ((SQLException) failure).getErrorCode() : "-") + "\t"
                    + utf16(failure instanceof SQLException ? ((SQLException) failure).getSQLState() : null) + "\t" + utf16(failure.getMessage());
            return input[0] + "\t" + diagnostics + "\t" + activeDuring + "\t" + idleDuring
                + "\t" + activeAfter + "\t" + idleAfter + "\t" + statementClosed + "\t" + connectionClosed
                + "\t" + same + "\t" + source.getActiveCount() + "\t" + source.getPoolingCount();
        } finally { lifecycle(test, "tearDown"); }
    }
    public static void main(String[] args) throws Exception {
        if (!"1.2.28".equals(com.alibaba.druid.VERSION.getVersionNumber())) { throw new IllegalArgumentException("version drift"); }
        byte[] bytes = Files.readAllBytes(Paths.get(args[0]));
        StringBuilder digest = new StringBuilder();
        for (byte value : MessageDigest.getInstance("SHA-256").digest(bytes)) { digest.append(String.format("%02x", value & 255)); }
        if (!"09dd147f9f41fa33a9950f67b23d3d21d54819ed873d61f81c11f8058a1b596d".equals(digest.toString())) { throw new IllegalArgumentException("corpus drift"); }
        List<String> lines = Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8);
        Set<String> expected = new HashSet<>();
        for (String name : new String[] {"DruidDataSourceTest_getConnection", "DruidDataSourceTest_recycle", "DruidDataSourceTest_recycle2"}) {
            for (Method method : Class.forName(PACKAGE + name).getDeclaredMethods()) {
                if (method.getName().startsWith("test") && method.getParameterCount() == 0) { expected.add(name + "." + method.getName()); }
            }
        }
        if (expected.size() != 6 || lines.size() != 7) { throw new IllegalArgumentException("original denominator drift"); }
        StringBuilder output = new StringBuilder("case_id\toutcome\terror_class\terror_code\tsql_state_utf16\tmessage_utf16\tactive_during\tidle_during\tactive_after_close\tidle_after_close\tstatement_closed\tconnection_closed\treplacement_same\tactive_final\tidle_final\n");
        for (String line : lines.subList(1, lines.size())) {
            String[] input = line.split("\t", -1);
            if (input.length != 9 || !input[0].equals(input[1] + "." + input[2]) || !expected.remove(input[0])) { throw new IllegalArgumentException("case drift"); }
            output.append(observe(input)).append('\n');
        }
        Files.write(Paths.get(args[1]), output.toString().getBytes(StandardCharsets.UTF_8));
    }
}
