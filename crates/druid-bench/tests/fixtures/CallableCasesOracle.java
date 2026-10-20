import com.alibaba.druid.bvt.pool.DruidPooledCallableStatementTest;
import com.alibaba.druid.mock.MockCallableStatement;
import com.alibaba.druid.pool.DruidDataSource;
import com.alibaba.druid.pool.DruidPooledConnection;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.sql.CallableStatement;
import java.sql.PreparedStatement;
import java.sql.SQLException;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;

/** 运行原始七例断言，再用相同 setup 与输入观测 Callable 的错误、身份和关闭状态。 */
public final class CallableCasesOracle {
    private CallableCasesOracle() {}
    private static void lifecycle(Object test, String method) throws Exception {
        Method entry = test.getClass().getDeclaredMethod(method);
        entry.setAccessible(true);
        entry.invoke(test);
    }
    private static DruidPooledCallableStatementTest start() throws Exception {
        DruidPooledCallableStatementTest test = new DruidPooledCallableStatementTest();
        lifecycle(test, "setUp");
        return test;
    }
    private static String utf16(String value) {
        if (Objects.isNull(value)) { return "-"; }
        StringBuilder result = new StringBuilder();
        for (int i = 0; i < value.length(); i++) { result.append(String.format("%04x", (int) value.charAt(i))); }
        return result.toString();
    }
    private static String observe(String[] input) throws Exception {
        DruidPooledCallableStatementTest original = start();
        try { original.getClass().getMethod(input[1]).invoke(original); }
        finally { lifecycle(original, "tearDown"); }
        DruidPooledCallableStatementTest test = start();
        try {
            Field field = test.getClass().getDeclaredField("dataSource");
            field.setAccessible(true);
            DruidDataSource source = (DruidDataSource) field.get(test);
            if (source.getInitialSize() != 1) { throw new IllegalArgumentException("initial size drift"); }
            if (!Boolean.parseBoolean(input[4])) { source.getProxyFilters().clear(); }
            DruidPooledConnection connection = source.getConnection();
            Object raw = connection.getConnection();
            CallableStatement statement = connection.prepareCall(input[2]);
            if (Boolean.parseBoolean(input[3])) { statement.execute(); }
            Field gate = test.getClass().getDeclaredField("throwError");
            gate.setAccessible(true);
            gate.setBoolean(test, Boolean.parseBoolean(input[5]));
            long before = source.getErrorCount();
            String trace = "-";
            Exception failure = null;
            try {
                if ("WAS_NULL".equals(input[6])) { trace = Boolean.toString(statement.wasNull()); }
                else if ("TYPED_INDEX".equals(input[6])) { statement.getObject(Integer.parseInt(input[7]), String.class); }
                else if ("TYPED_NAME".equals(input[6])) { statement.getObject(input[7], String.class); }
                else if ("UNWRAP".equals(input[6])) {
                    Object selected = "callable".equals(input[7]) ? statement.unwrap(CallableStatement.class) : statement.unwrap(PreparedStatement.class);
                    Object callable = statement.unwrap(CallableStatement.class);
                    trace = "role_raw=" + (Objects.nonNull(selected) && selected.getClass() == MockCallableStatement.class) + ",callable_raw=" + (Objects.nonNull(callable) && callable.getClass() == MockCallableStatement.class) + ",logical=" + (selected == statement && callable == statement);
                } else { throw new IllegalArgumentException("unknown operation"); }
            } catch (Exception error) { failure = error; }
            long after = source.getErrorCount();
            int active = source.getActiveCount();
            int idle = source.getPoolingCount();
            statement.close(); connection.close();
            boolean statementClosed = statement.isClosed();
            boolean connectionClosed = connection.isClosed();
            int activeAfter = source.getActiveCount();
            int idleAfter = source.getPoolingCount();
            DruidPooledConnection next = source.getConnection();
            boolean same = raw == next.getConnection();
            next.close();
            String error = Objects.isNull(failure) ? "RETURN\t-\t-\t-\t-" : "THROW\t" + failure.getClass().getName() + "\t" + (failure instanceof SQLException ? ((SQLException) failure).getErrorCode() : "-") + "\t" + utf16(failure instanceof SQLException ? ((SQLException) failure).getSQLState() : null) + "\t" + utf16(failure.getMessage());
            return input[0] + "\t" + trace + "\t" + error + "\t" + before + "\t" + after + "\t" + active + "\t" + idle + "\t" + statementClosed + "\t" + connectionClosed + "\t" + activeAfter + "\t" + idleAfter + "\t" + same;
        } finally { lifecycle(test, "tearDown"); }
    }
    public static void main(String[] args) throws Exception {
        if (!"1.2.28".equals(com.alibaba.druid.VERSION.getVersionNumber())) { throw new IllegalArgumentException("version drift"); }
        StringBuilder digest = new StringBuilder();
        for (byte value : MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(Paths.get(args[0])))) { digest.append(String.format("%02x", value & 255)); }
        if (!"1c207a287093514fcb7814bd420e84f28620522a5d8b8ad5bb5c040b57021940".equals(digest.toString())) { throw new IllegalArgumentException("corpus drift"); }
        Set<String> methods = new HashSet<>();
        for (Method method : DruidPooledCallableStatementTest.class.getDeclaredMethods()) {
            if (method.getName().startsWith("test") && method.getParameterCount() == 0) { methods.add(method.getName()); }
        }
        List<String> lines = Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8);
        if (methods.size() != 7 || lines.size() != 8) { throw new IllegalArgumentException("denominator drift"); }
        StringBuilder output = new StringBuilder("case_id\ttrace\toutcome\terror_class\terror_code\tsql_state_utf16\tmessage_utf16\terrors_before\terrors_after\tactive_during\tidle_during\tstatement_closed\tconnection_closed\tactive_after\tidle_after\treused\n");
        for (String line : lines.subList(1, lines.size())) {
            String[] input = line.split("\t", -1);
            if (input.length != 8 || !methods.remove(input[1]) || !input[0].equals("DruidPooledCallableStatementTest." + input[1])) { throw new IllegalArgumentException("input identity drift"); }
            output.append(observe(input)).append('\n');
        }
        Files.write(Paths.get(args[1]), output.toString().getBytes(StandardCharsets.UTF_8));
    }
}
